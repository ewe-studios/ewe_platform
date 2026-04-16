//! ServicenetworkingProvider - State-aware servicenetworking API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       servicenetworking API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::servicenetworking::{
    servicenetworking_operations_cancel_builder, servicenetworking_operations_cancel_task,
    servicenetworking_operations_delete_builder, servicenetworking_operations_delete_task,
    servicenetworking_operations_get_builder, servicenetworking_operations_get_task,
    servicenetworking_operations_list_builder, servicenetworking_operations_list_task,
    servicenetworking_services_add_subnetwork_builder, servicenetworking_services_add_subnetwork_task,
    servicenetworking_services_disable_vpc_service_controls_builder, servicenetworking_services_disable_vpc_service_controls_task,
    servicenetworking_services_enable_vpc_service_controls_builder, servicenetworking_services_enable_vpc_service_controls_task,
    servicenetworking_services_search_range_builder, servicenetworking_services_search_range_task,
    servicenetworking_services_validate_builder, servicenetworking_services_validate_task,
    servicenetworking_services_connections_create_builder, servicenetworking_services_connections_create_task,
    servicenetworking_services_connections_delete_connection_builder, servicenetworking_services_connections_delete_connection_task,
    servicenetworking_services_connections_list_builder, servicenetworking_services_connections_list_task,
    servicenetworking_services_connections_patch_builder, servicenetworking_services_connections_patch_task,
    servicenetworking_services_dns_record_sets_add_builder, servicenetworking_services_dns_record_sets_add_task,
    servicenetworking_services_dns_record_sets_get_builder, servicenetworking_services_dns_record_sets_get_task,
    servicenetworking_services_dns_record_sets_list_builder, servicenetworking_services_dns_record_sets_list_task,
    servicenetworking_services_dns_record_sets_remove_builder, servicenetworking_services_dns_record_sets_remove_task,
    servicenetworking_services_dns_record_sets_update_builder, servicenetworking_services_dns_record_sets_update_task,
    servicenetworking_services_dns_zones_add_builder, servicenetworking_services_dns_zones_add_task,
    servicenetworking_services_dns_zones_remove_builder, servicenetworking_services_dns_zones_remove_task,
    servicenetworking_services_projects_global_networks_get_builder, servicenetworking_services_projects_global_networks_get_task,
    servicenetworking_services_projects_global_networks_get_vpc_service_controls_builder, servicenetworking_services_projects_global_networks_get_vpc_service_controls_task,
    servicenetworking_services_projects_global_networks_update_consumer_config_builder, servicenetworking_services_projects_global_networks_update_consumer_config_task,
    servicenetworking_services_projects_global_networks_dns_zones_get_builder, servicenetworking_services_projects_global_networks_dns_zones_get_task,
    servicenetworking_services_projects_global_networks_dns_zones_list_builder, servicenetworking_services_projects_global_networks_dns_zones_list_task,
    servicenetworking_services_projects_global_networks_peered_dns_domains_create_builder, servicenetworking_services_projects_global_networks_peered_dns_domains_create_task,
    servicenetworking_services_projects_global_networks_peered_dns_domains_delete_builder, servicenetworking_services_projects_global_networks_peered_dns_domains_delete_task,
    servicenetworking_services_projects_global_networks_peered_dns_domains_list_builder, servicenetworking_services_projects_global_networks_peered_dns_domains_list_task,
    servicenetworking_services_roles_add_builder, servicenetworking_services_roles_add_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::servicenetworking::ConsumerConfig;
use crate::providers::gcp::clients::servicenetworking::DnsRecordSet;
use crate::providers::gcp::clients::servicenetworking::Empty;
use crate::providers::gcp::clients::servicenetworking::GetDnsZoneResponse;
use crate::providers::gcp::clients::servicenetworking::ListConnectionsResponse;
use crate::providers::gcp::clients::servicenetworking::ListDnsRecordSetsResponse;
use crate::providers::gcp::clients::servicenetworking::ListDnsZonesResponse;
use crate::providers::gcp::clients::servicenetworking::ListOperationsResponse;
use crate::providers::gcp::clients::servicenetworking::ListPeeredDnsDomainsResponse;
use crate::providers::gcp::clients::servicenetworking::Operation;
use crate::providers::gcp::clients::servicenetworking::ValidateConsumerConfigResponse;
use crate::providers::gcp::clients::servicenetworking::VpcServiceControls;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingOperationsCancelArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingOperationsDeleteArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingOperationsGetArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingOperationsListArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingServicesAddSubnetworkArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingServicesConnectionsCreateArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingServicesConnectionsDeleteConnectionArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingServicesConnectionsListArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingServicesConnectionsPatchArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingServicesDisableVpcServiceControlsArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingServicesDnsRecordSetsAddArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingServicesDnsRecordSetsGetArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingServicesDnsRecordSetsListArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingServicesDnsRecordSetsRemoveArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingServicesDnsRecordSetsUpdateArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingServicesDnsZonesAddArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingServicesDnsZonesRemoveArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingServicesEnableVpcServiceControlsArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingServicesProjectsGlobalNetworksDnsZonesGetArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingServicesProjectsGlobalNetworksDnsZonesListArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingServicesProjectsGlobalNetworksGetArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingServicesProjectsGlobalNetworksGetVpcServiceControlsArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingServicesProjectsGlobalNetworksPeeredDnsDomainsCreateArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingServicesProjectsGlobalNetworksPeeredDnsDomainsDeleteArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingServicesProjectsGlobalNetworksPeeredDnsDomainsListArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingServicesProjectsGlobalNetworksUpdateConsumerConfigArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingServicesRolesAddArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingServicesSearchRangeArgs;
use crate::providers::gcp::clients::servicenetworking::ServicenetworkingServicesValidateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ServicenetworkingProvider with automatic state tracking.
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
/// let provider = ServicenetworkingProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct ServicenetworkingProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> ServicenetworkingProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new ServicenetworkingProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new ServicenetworkingProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Servicenetworking operations cancel.
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
    pub fn servicenetworking_operations_cancel(
        &self,
        args: &ServicenetworkingOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking operations delete.
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
    pub fn servicenetworking_operations_delete(
        &self,
        args: &ServicenetworkingOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking operations get.
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
    pub fn servicenetworking_operations_get(
        &self,
        args: &ServicenetworkingOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking operations list.
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
    pub fn servicenetworking_operations_list(
        &self,
        args: &ServicenetworkingOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_operations_list_builder(
            &self.http_client,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking services add subnetwork.
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
    pub fn servicenetworking_services_add_subnetwork(
        &self,
        args: &ServicenetworkingServicesAddSubnetworkArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_services_add_subnetwork_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_services_add_subnetwork_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking services disable vpc service controls.
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
    pub fn servicenetworking_services_disable_vpc_service_controls(
        &self,
        args: &ServicenetworkingServicesDisableVpcServiceControlsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_services_disable_vpc_service_controls_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_services_disable_vpc_service_controls_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking services enable vpc service controls.
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
    pub fn servicenetworking_services_enable_vpc_service_controls(
        &self,
        args: &ServicenetworkingServicesEnableVpcServiceControlsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_services_enable_vpc_service_controls_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_services_enable_vpc_service_controls_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking services search range.
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
    pub fn servicenetworking_services_search_range(
        &self,
        args: &ServicenetworkingServicesSearchRangeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_services_search_range_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_services_search_range_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking services validate.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ValidateConsumerConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn servicenetworking_services_validate(
        &self,
        args: &ServicenetworkingServicesValidateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ValidateConsumerConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_services_validate_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_services_validate_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking services connections create.
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
    pub fn servicenetworking_services_connections_create(
        &self,
        args: &ServicenetworkingServicesConnectionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_services_connections_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_services_connections_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking services connections delete connection.
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
    pub fn servicenetworking_services_connections_delete_connection(
        &self,
        args: &ServicenetworkingServicesConnectionsDeleteConnectionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_services_connections_delete_connection_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_services_connections_delete_connection_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking services connections list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListConnectionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn servicenetworking_services_connections_list(
        &self,
        args: &ServicenetworkingServicesConnectionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListConnectionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_services_connections_list_builder(
            &self.http_client,
            &args.parent,
            &args.network,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_services_connections_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking services connections patch.
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
    pub fn servicenetworking_services_connections_patch(
        &self,
        args: &ServicenetworkingServicesConnectionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_services_connections_patch_builder(
            &self.http_client,
            &args.name,
            &args.force,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_services_connections_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking services dns record sets add.
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
    pub fn servicenetworking_services_dns_record_sets_add(
        &self,
        args: &ServicenetworkingServicesDnsRecordSetsAddArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_services_dns_record_sets_add_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_services_dns_record_sets_add_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking services dns record sets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DnsRecordSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn servicenetworking_services_dns_record_sets_get(
        &self,
        args: &ServicenetworkingServicesDnsRecordSetsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DnsRecordSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_services_dns_record_sets_get_builder(
            &self.http_client,
            &args.parent,
            &args.consumerNetwork,
            &args.domain,
            &args.type_rs,
            &args.zone,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_services_dns_record_sets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking services dns record sets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDnsRecordSetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn servicenetworking_services_dns_record_sets_list(
        &self,
        args: &ServicenetworkingServicesDnsRecordSetsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDnsRecordSetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_services_dns_record_sets_list_builder(
            &self.http_client,
            &args.parent,
            &args.consumerNetwork,
            &args.zone,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_services_dns_record_sets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking services dns record sets remove.
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
    pub fn servicenetworking_services_dns_record_sets_remove(
        &self,
        args: &ServicenetworkingServicesDnsRecordSetsRemoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_services_dns_record_sets_remove_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_services_dns_record_sets_remove_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking services dns record sets update.
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
    pub fn servicenetworking_services_dns_record_sets_update(
        &self,
        args: &ServicenetworkingServicesDnsRecordSetsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_services_dns_record_sets_update_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_services_dns_record_sets_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking services dns zones add.
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
    pub fn servicenetworking_services_dns_zones_add(
        &self,
        args: &ServicenetworkingServicesDnsZonesAddArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_services_dns_zones_add_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_services_dns_zones_add_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking services dns zones remove.
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
    pub fn servicenetworking_services_dns_zones_remove(
        &self,
        args: &ServicenetworkingServicesDnsZonesRemoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_services_dns_zones_remove_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_services_dns_zones_remove_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking services projects global networks get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConsumerConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn servicenetworking_services_projects_global_networks_get(
        &self,
        args: &ServicenetworkingServicesProjectsGlobalNetworksGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConsumerConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_services_projects_global_networks_get_builder(
            &self.http_client,
            &args.name,
            &args.includeUsedIpRanges,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_services_projects_global_networks_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking services projects global networks get vpc service controls.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VpcServiceControls result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn servicenetworking_services_projects_global_networks_get_vpc_service_controls(
        &self,
        args: &ServicenetworkingServicesProjectsGlobalNetworksGetVpcServiceControlsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VpcServiceControls, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_services_projects_global_networks_get_vpc_service_controls_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_services_projects_global_networks_get_vpc_service_controls_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking services projects global networks update consumer config.
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
    pub fn servicenetworking_services_projects_global_networks_update_consumer_config(
        &self,
        args: &ServicenetworkingServicesProjectsGlobalNetworksUpdateConsumerConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_services_projects_global_networks_update_consumer_config_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_services_projects_global_networks_update_consumer_config_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking services projects global networks dns zones get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetDnsZoneResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn servicenetworking_services_projects_global_networks_dns_zones_get(
        &self,
        args: &ServicenetworkingServicesProjectsGlobalNetworksDnsZonesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetDnsZoneResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_services_projects_global_networks_dns_zones_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_services_projects_global_networks_dns_zones_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking services projects global networks dns zones list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListDnsZonesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn servicenetworking_services_projects_global_networks_dns_zones_list(
        &self,
        args: &ServicenetworkingServicesProjectsGlobalNetworksDnsZonesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListDnsZonesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_services_projects_global_networks_dns_zones_list_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_services_projects_global_networks_dns_zones_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking services projects global networks peered dns domains create.
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
    pub fn servicenetworking_services_projects_global_networks_peered_dns_domains_create(
        &self,
        args: &ServicenetworkingServicesProjectsGlobalNetworksPeeredDnsDomainsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_services_projects_global_networks_peered_dns_domains_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_services_projects_global_networks_peered_dns_domains_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking services projects global networks peered dns domains delete.
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
    pub fn servicenetworking_services_projects_global_networks_peered_dns_domains_delete(
        &self,
        args: &ServicenetworkingServicesProjectsGlobalNetworksPeeredDnsDomainsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_services_projects_global_networks_peered_dns_domains_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_services_projects_global_networks_peered_dns_domains_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking services projects global networks peered dns domains list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPeeredDnsDomainsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn servicenetworking_services_projects_global_networks_peered_dns_domains_list(
        &self,
        args: &ServicenetworkingServicesProjectsGlobalNetworksPeeredDnsDomainsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPeeredDnsDomainsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_services_projects_global_networks_peered_dns_domains_list_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_services_projects_global_networks_peered_dns_domains_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Servicenetworking services roles add.
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
    pub fn servicenetworking_services_roles_add(
        &self,
        args: &ServicenetworkingServicesRolesAddArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = servicenetworking_services_roles_add_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = servicenetworking_services_roles_add_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
