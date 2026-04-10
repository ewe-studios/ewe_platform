//! NetworkconnectivityProvider - State-aware networkconnectivity API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       networkconnectivity API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::networkconnectivity::{
    networkconnectivity_projects_locations_check_consumer_config_builder, networkconnectivity_projects_locations_check_consumer_config_task,
    networkconnectivity_projects_locations_automated_dns_records_create_builder, networkconnectivity_projects_locations_automated_dns_records_create_task,
    networkconnectivity_projects_locations_automated_dns_records_delete_builder, networkconnectivity_projects_locations_automated_dns_records_delete_task,
    networkconnectivity_projects_locations_global_hubs_accept_spoke_builder, networkconnectivity_projects_locations_global_hubs_accept_spoke_task,
    networkconnectivity_projects_locations_global_hubs_accept_spoke_update_builder, networkconnectivity_projects_locations_global_hubs_accept_spoke_update_task,
    networkconnectivity_projects_locations_global_hubs_create_builder, networkconnectivity_projects_locations_global_hubs_create_task,
    networkconnectivity_projects_locations_global_hubs_delete_builder, networkconnectivity_projects_locations_global_hubs_delete_task,
    networkconnectivity_projects_locations_global_hubs_patch_builder, networkconnectivity_projects_locations_global_hubs_patch_task,
    networkconnectivity_projects_locations_global_hubs_reject_spoke_builder, networkconnectivity_projects_locations_global_hubs_reject_spoke_task,
    networkconnectivity_projects_locations_global_hubs_reject_spoke_update_builder, networkconnectivity_projects_locations_global_hubs_reject_spoke_update_task,
    networkconnectivity_projects_locations_global_hubs_set_iam_policy_builder, networkconnectivity_projects_locations_global_hubs_set_iam_policy_task,
    networkconnectivity_projects_locations_global_hubs_test_iam_permissions_builder, networkconnectivity_projects_locations_global_hubs_test_iam_permissions_task,
    networkconnectivity_projects_locations_global_hubs_groups_patch_builder, networkconnectivity_projects_locations_global_hubs_groups_patch_task,
    networkconnectivity_projects_locations_global_hubs_groups_set_iam_policy_builder, networkconnectivity_projects_locations_global_hubs_groups_set_iam_policy_task,
    networkconnectivity_projects_locations_global_hubs_groups_test_iam_permissions_builder, networkconnectivity_projects_locations_global_hubs_groups_test_iam_permissions_task,
    networkconnectivity_projects_locations_global_policy_based_routes_create_builder, networkconnectivity_projects_locations_global_policy_based_routes_create_task,
    networkconnectivity_projects_locations_global_policy_based_routes_delete_builder, networkconnectivity_projects_locations_global_policy_based_routes_delete_task,
    networkconnectivity_projects_locations_global_policy_based_routes_set_iam_policy_builder, networkconnectivity_projects_locations_global_policy_based_routes_set_iam_policy_task,
    networkconnectivity_projects_locations_global_policy_based_routes_test_iam_permissions_builder, networkconnectivity_projects_locations_global_policy_based_routes_test_iam_permissions_task,
    networkconnectivity_projects_locations_internal_ranges_create_builder, networkconnectivity_projects_locations_internal_ranges_create_task,
    networkconnectivity_projects_locations_internal_ranges_delete_builder, networkconnectivity_projects_locations_internal_ranges_delete_task,
    networkconnectivity_projects_locations_internal_ranges_patch_builder, networkconnectivity_projects_locations_internal_ranges_patch_task,
    networkconnectivity_projects_locations_internal_ranges_set_iam_policy_builder, networkconnectivity_projects_locations_internal_ranges_set_iam_policy_task,
    networkconnectivity_projects_locations_internal_ranges_test_iam_permissions_builder, networkconnectivity_projects_locations_internal_ranges_test_iam_permissions_task,
    networkconnectivity_projects_locations_multicloud_data_transfer_configs_create_builder, networkconnectivity_projects_locations_multicloud_data_transfer_configs_create_task,
    networkconnectivity_projects_locations_multicloud_data_transfer_configs_delete_builder, networkconnectivity_projects_locations_multicloud_data_transfer_configs_delete_task,
    networkconnectivity_projects_locations_multicloud_data_transfer_configs_patch_builder, networkconnectivity_projects_locations_multicloud_data_transfer_configs_patch_task,
    networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_create_builder, networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_create_task,
    networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_delete_builder, networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_delete_task,
    networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_patch_builder, networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_patch_task,
    networkconnectivity_projects_locations_operations_cancel_builder, networkconnectivity_projects_locations_operations_cancel_task,
    networkconnectivity_projects_locations_operations_delete_builder, networkconnectivity_projects_locations_operations_delete_task,
    networkconnectivity_projects_locations_regional_endpoints_create_builder, networkconnectivity_projects_locations_regional_endpoints_create_task,
    networkconnectivity_projects_locations_regional_endpoints_delete_builder, networkconnectivity_projects_locations_regional_endpoints_delete_task,
    networkconnectivity_projects_locations_service_classes_delete_builder, networkconnectivity_projects_locations_service_classes_delete_task,
    networkconnectivity_projects_locations_service_classes_patch_builder, networkconnectivity_projects_locations_service_classes_patch_task,
    networkconnectivity_projects_locations_service_connection_maps_create_builder, networkconnectivity_projects_locations_service_connection_maps_create_task,
    networkconnectivity_projects_locations_service_connection_maps_delete_builder, networkconnectivity_projects_locations_service_connection_maps_delete_task,
    networkconnectivity_projects_locations_service_connection_maps_patch_builder, networkconnectivity_projects_locations_service_connection_maps_patch_task,
    networkconnectivity_projects_locations_service_connection_policies_create_builder, networkconnectivity_projects_locations_service_connection_policies_create_task,
    networkconnectivity_projects_locations_service_connection_policies_delete_builder, networkconnectivity_projects_locations_service_connection_policies_delete_task,
    networkconnectivity_projects_locations_service_connection_policies_patch_builder, networkconnectivity_projects_locations_service_connection_policies_patch_task,
    networkconnectivity_projects_locations_service_connection_tokens_create_builder, networkconnectivity_projects_locations_service_connection_tokens_create_task,
    networkconnectivity_projects_locations_service_connection_tokens_delete_builder, networkconnectivity_projects_locations_service_connection_tokens_delete_task,
    networkconnectivity_projects_locations_spokes_create_builder, networkconnectivity_projects_locations_spokes_create_task,
    networkconnectivity_projects_locations_spokes_delete_builder, networkconnectivity_projects_locations_spokes_delete_task,
    networkconnectivity_projects_locations_spokes_patch_builder, networkconnectivity_projects_locations_spokes_patch_task,
    networkconnectivity_projects_locations_spokes_set_iam_policy_builder, networkconnectivity_projects_locations_spokes_set_iam_policy_task,
    networkconnectivity_projects_locations_spokes_test_iam_permissions_builder, networkconnectivity_projects_locations_spokes_test_iam_permissions_task,
    networkconnectivity_projects_locations_transports_create_builder, networkconnectivity_projects_locations_transports_create_task,
    networkconnectivity_projects_locations_transports_delete_builder, networkconnectivity_projects_locations_transports_delete_task,
    networkconnectivity_projects_locations_transports_patch_builder, networkconnectivity_projects_locations_transports_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::networkconnectivity::CheckConsumerConfigResponse;
use crate::providers::gcp::clients::networkconnectivity::Empty;
use crate::providers::gcp::clients::networkconnectivity::GoogleLongrunningOperation;
use crate::providers::gcp::clients::networkconnectivity::Policy;
use crate::providers::gcp::clients::networkconnectivity::TestIamPermissionsResponse;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsAutomatedDnsRecordsCreateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsAutomatedDnsRecordsDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsCheckConsumerConfigArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsAcceptSpokeArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsAcceptSpokeUpdateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsCreateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsGroupsPatchArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsGroupsSetIamPolicyArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsGroupsTestIamPermissionsArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsPatchArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsRejectSpokeArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsRejectSpokeUpdateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsSetIamPolicyArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalHubsTestIamPermissionsArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalPolicyBasedRoutesCreateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalPolicyBasedRoutesDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalPolicyBasedRoutesSetIamPolicyArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsGlobalPolicyBasedRoutesTestIamPermissionsArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsInternalRangesCreateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsInternalRangesDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsInternalRangesPatchArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsInternalRangesSetIamPolicyArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsInternalRangesTestIamPermissionsArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsCreateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsDestinationsCreateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsDestinationsDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsDestinationsPatchArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsPatchArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsRegionalEndpointsCreateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsRegionalEndpointsDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceClassesDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceClassesPatchArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceConnectionMapsCreateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceConnectionMapsDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceConnectionMapsPatchArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceConnectionPoliciesCreateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceConnectionPoliciesDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceConnectionPoliciesPatchArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceConnectionTokensCreateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsServiceConnectionTokensDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsSpokesCreateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsSpokesDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsSpokesPatchArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsSpokesSetIamPolicyArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsSpokesTestIamPermissionsArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsTransportsCreateArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsTransportsDeleteArgs;
use crate::providers::gcp::clients::networkconnectivity::NetworkconnectivityProjectsLocationsTransportsPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// NetworkconnectivityProvider with automatic state tracking.
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
/// let provider = NetworkconnectivityProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct NetworkconnectivityProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> NetworkconnectivityProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new NetworkconnectivityProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Networkconnectivity projects locations check consumer config.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CheckConsumerConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_check_consumer_config(
        &self,
        args: &NetworkconnectivityProjectsLocationsCheckConsumerConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CheckConsumerConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_check_consumer_config_builder(
            &self.http_client,
            &args.location,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_check_consumer_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations automated dns records create.
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
    pub fn networkconnectivity_projects_locations_automated_dns_records_create(
        &self,
        args: &NetworkconnectivityProjectsLocationsAutomatedDnsRecordsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_automated_dns_records_create_builder(
            &self.http_client,
            &args.parent,
            &args.automatedDnsRecordId,
            &args.insertMode,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_automated_dns_records_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations automated dns records delete.
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
    pub fn networkconnectivity_projects_locations_automated_dns_records_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsAutomatedDnsRecordsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_automated_dns_records_delete_builder(
            &self.http_client,
            &args.name,
            &args.deleteMode,
            &args.etag,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_automated_dns_records_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs accept spoke.
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
    pub fn networkconnectivity_projects_locations_global_hubs_accept_spoke(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsAcceptSpokeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_accept_spoke_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_accept_spoke_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs accept spoke update.
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
    pub fn networkconnectivity_projects_locations_global_hubs_accept_spoke_update(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsAcceptSpokeUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_accept_spoke_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_accept_spoke_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs create.
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
    pub fn networkconnectivity_projects_locations_global_hubs_create(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_create_builder(
            &self.http_client,
            &args.parent,
            &args.hubId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs delete.
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
    pub fn networkconnectivity_projects_locations_global_hubs_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs patch.
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
    pub fn networkconnectivity_projects_locations_global_hubs_patch(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs reject spoke.
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
    pub fn networkconnectivity_projects_locations_global_hubs_reject_spoke(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsRejectSpokeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_reject_spoke_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_reject_spoke_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs reject spoke update.
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
    pub fn networkconnectivity_projects_locations_global_hubs_reject_spoke_update(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsRejectSpokeUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_reject_spoke_update_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_reject_spoke_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs set iam policy.
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
    pub fn networkconnectivity_projects_locations_global_hubs_set_iam_policy(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_global_hubs_test_iam_permissions(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs groups patch.
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
    pub fn networkconnectivity_projects_locations_global_hubs_groups_patch(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsGroupsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_groups_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_groups_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs groups set iam policy.
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
    pub fn networkconnectivity_projects_locations_global_hubs_groups_set_iam_policy(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsGroupsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_groups_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_groups_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global hubs groups test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_global_hubs_groups_test_iam_permissions(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalHubsGroupsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_hubs_groups_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_hubs_groups_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global policy based routes create.
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
    pub fn networkconnectivity_projects_locations_global_policy_based_routes_create(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalPolicyBasedRoutesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_policy_based_routes_create_builder(
            &self.http_client,
            &args.parent,
            &args.policyBasedRouteId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_policy_based_routes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global policy based routes delete.
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
    pub fn networkconnectivity_projects_locations_global_policy_based_routes_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalPolicyBasedRoutesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_policy_based_routes_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_policy_based_routes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global policy based routes set iam policy.
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
    pub fn networkconnectivity_projects_locations_global_policy_based_routes_set_iam_policy(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalPolicyBasedRoutesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_policy_based_routes_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_policy_based_routes_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations global policy based routes test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_global_policy_based_routes_test_iam_permissions(
        &self,
        args: &NetworkconnectivityProjectsLocationsGlobalPolicyBasedRoutesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_global_policy_based_routes_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_global_policy_based_routes_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations internal ranges create.
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
    pub fn networkconnectivity_projects_locations_internal_ranges_create(
        &self,
        args: &NetworkconnectivityProjectsLocationsInternalRangesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_internal_ranges_create_builder(
            &self.http_client,
            &args.parent,
            &args.internalRangeId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_internal_ranges_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations internal ranges delete.
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
    pub fn networkconnectivity_projects_locations_internal_ranges_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsInternalRangesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_internal_ranges_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_internal_ranges_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations internal ranges patch.
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
    pub fn networkconnectivity_projects_locations_internal_ranges_patch(
        &self,
        args: &NetworkconnectivityProjectsLocationsInternalRangesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_internal_ranges_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_internal_ranges_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations internal ranges set iam policy.
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
    pub fn networkconnectivity_projects_locations_internal_ranges_set_iam_policy(
        &self,
        args: &NetworkconnectivityProjectsLocationsInternalRangesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_internal_ranges_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_internal_ranges_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations internal ranges test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_internal_ranges_test_iam_permissions(
        &self,
        args: &NetworkconnectivityProjectsLocationsInternalRangesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_internal_ranges_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_internal_ranges_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations multicloud data transfer configs create.
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
    pub fn networkconnectivity_projects_locations_multicloud_data_transfer_configs_create(
        &self,
        args: &NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_multicloud_data_transfer_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.multicloudDataTransferConfigId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_multicloud_data_transfer_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations multicloud data transfer configs delete.
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
    pub fn networkconnectivity_projects_locations_multicloud_data_transfer_configs_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_multicloud_data_transfer_configs_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_multicloud_data_transfer_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations multicloud data transfer configs patch.
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
    pub fn networkconnectivity_projects_locations_multicloud_data_transfer_configs_patch(
        &self,
        args: &NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_multicloud_data_transfer_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_multicloud_data_transfer_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations multicloud data transfer configs destinations create.
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
    pub fn networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_create(
        &self,
        args: &NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsDestinationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_create_builder(
            &self.http_client,
            &args.parent,
            &args.destinationId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations multicloud data transfer configs destinations delete.
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
    pub fn networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsDestinationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations multicloud data transfer configs destinations patch.
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
    pub fn networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_patch(
        &self,
        args: &NetworkconnectivityProjectsLocationsMulticloudDataTransferConfigsDestinationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_multicloud_data_transfer_configs_destinations_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations operations cancel.
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
    pub fn networkconnectivity_projects_locations_operations_cancel(
        &self,
        args: &NetworkconnectivityProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations operations delete.
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
    pub fn networkconnectivity_projects_locations_operations_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations regional endpoints create.
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
    pub fn networkconnectivity_projects_locations_regional_endpoints_create(
        &self,
        args: &NetworkconnectivityProjectsLocationsRegionalEndpointsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_regional_endpoints_create_builder(
            &self.http_client,
            &args.parent,
            &args.regionalEndpointId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_regional_endpoints_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations regional endpoints delete.
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
    pub fn networkconnectivity_projects_locations_regional_endpoints_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsRegionalEndpointsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_regional_endpoints_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_regional_endpoints_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service classes delete.
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
    pub fn networkconnectivity_projects_locations_service_classes_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceClassesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_classes_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_classes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service classes patch.
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
    pub fn networkconnectivity_projects_locations_service_classes_patch(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceClassesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_classes_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_classes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service connection maps create.
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
    pub fn networkconnectivity_projects_locations_service_connection_maps_create(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceConnectionMapsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_connection_maps_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.serviceConnectionMapId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_connection_maps_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service connection maps delete.
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
    pub fn networkconnectivity_projects_locations_service_connection_maps_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceConnectionMapsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_connection_maps_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_connection_maps_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service connection maps patch.
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
    pub fn networkconnectivity_projects_locations_service_connection_maps_patch(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceConnectionMapsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_connection_maps_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_connection_maps_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service connection policies create.
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
    pub fn networkconnectivity_projects_locations_service_connection_policies_create(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceConnectionPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_connection_policies_create_builder(
            &self.http_client,
            &args.parent,
            &args.autoSubnetworkConfig.allocRangeSpace,
            &args.autoSubnetworkConfig.ipStack,
            &args.autoSubnetworkConfig.prefixLength,
            &args.requestId,
            &args.serviceConnectionPolicyId,
            &args.subnetworkMode,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_connection_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service connection policies delete.
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
    pub fn networkconnectivity_projects_locations_service_connection_policies_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceConnectionPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_connection_policies_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_connection_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service connection policies patch.
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
    pub fn networkconnectivity_projects_locations_service_connection_policies_patch(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceConnectionPoliciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_connection_policies_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_connection_policies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service connection tokens create.
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
    pub fn networkconnectivity_projects_locations_service_connection_tokens_create(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceConnectionTokensCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_connection_tokens_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.serviceConnectionTokenId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_connection_tokens_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations service connection tokens delete.
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
    pub fn networkconnectivity_projects_locations_service_connection_tokens_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsServiceConnectionTokensDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_service_connection_tokens_delete_builder(
            &self.http_client,
            &args.name,
            &args.etag,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_service_connection_tokens_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations spokes create.
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
    pub fn networkconnectivity_projects_locations_spokes_create(
        &self,
        args: &NetworkconnectivityProjectsLocationsSpokesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_spokes_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.spokeId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_spokes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations spokes delete.
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
    pub fn networkconnectivity_projects_locations_spokes_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsSpokesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_spokes_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_spokes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations spokes patch.
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
    pub fn networkconnectivity_projects_locations_spokes_patch(
        &self,
        args: &NetworkconnectivityProjectsLocationsSpokesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_spokes_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_spokes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations spokes set iam policy.
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
    pub fn networkconnectivity_projects_locations_spokes_set_iam_policy(
        &self,
        args: &NetworkconnectivityProjectsLocationsSpokesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_spokes_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_spokes_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations spokes test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn networkconnectivity_projects_locations_spokes_test_iam_permissions(
        &self,
        args: &NetworkconnectivityProjectsLocationsSpokesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_spokes_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_spokes_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations transports create.
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
    pub fn networkconnectivity_projects_locations_transports_create(
        &self,
        args: &NetworkconnectivityProjectsLocationsTransportsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_transports_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.transportId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_transports_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations transports delete.
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
    pub fn networkconnectivity_projects_locations_transports_delete(
        &self,
        args: &NetworkconnectivityProjectsLocationsTransportsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_transports_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_transports_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkconnectivity projects locations transports patch.
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
    pub fn networkconnectivity_projects_locations_transports_patch(
        &self,
        args: &NetworkconnectivityProjectsLocationsTransportsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkconnectivity_projects_locations_transports_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkconnectivity_projects_locations_transports_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
