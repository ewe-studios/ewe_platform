//! NetworkmanagementProvider - State-aware networkmanagement API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       networkmanagement API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::networkmanagement::{
    networkmanagement_organizations_locations_get_builder, networkmanagement_organizations_locations_get_task,
    networkmanagement_organizations_locations_list_builder, networkmanagement_organizations_locations_list_task,
    networkmanagement_organizations_locations_global_operations_cancel_builder, networkmanagement_organizations_locations_global_operations_cancel_task,
    networkmanagement_organizations_locations_global_operations_delete_builder, networkmanagement_organizations_locations_global_operations_delete_task,
    networkmanagement_organizations_locations_global_operations_get_builder, networkmanagement_organizations_locations_global_operations_get_task,
    networkmanagement_organizations_locations_global_operations_list_builder, networkmanagement_organizations_locations_global_operations_list_task,
    networkmanagement_organizations_locations_vpc_flow_logs_configs_create_builder, networkmanagement_organizations_locations_vpc_flow_logs_configs_create_task,
    networkmanagement_organizations_locations_vpc_flow_logs_configs_delete_builder, networkmanagement_organizations_locations_vpc_flow_logs_configs_delete_task,
    networkmanagement_organizations_locations_vpc_flow_logs_configs_get_builder, networkmanagement_organizations_locations_vpc_flow_logs_configs_get_task,
    networkmanagement_organizations_locations_vpc_flow_logs_configs_list_builder, networkmanagement_organizations_locations_vpc_flow_logs_configs_list_task,
    networkmanagement_organizations_locations_vpc_flow_logs_configs_patch_builder, networkmanagement_organizations_locations_vpc_flow_logs_configs_patch_task,
    networkmanagement_projects_locations_get_builder, networkmanagement_projects_locations_get_task,
    networkmanagement_projects_locations_list_builder, networkmanagement_projects_locations_list_task,
    networkmanagement_projects_locations_global_connectivity_tests_create_builder, networkmanagement_projects_locations_global_connectivity_tests_create_task,
    networkmanagement_projects_locations_global_connectivity_tests_delete_builder, networkmanagement_projects_locations_global_connectivity_tests_delete_task,
    networkmanagement_projects_locations_global_connectivity_tests_get_builder, networkmanagement_projects_locations_global_connectivity_tests_get_task,
    networkmanagement_projects_locations_global_connectivity_tests_get_iam_policy_builder, networkmanagement_projects_locations_global_connectivity_tests_get_iam_policy_task,
    networkmanagement_projects_locations_global_connectivity_tests_list_builder, networkmanagement_projects_locations_global_connectivity_tests_list_task,
    networkmanagement_projects_locations_global_connectivity_tests_patch_builder, networkmanagement_projects_locations_global_connectivity_tests_patch_task,
    networkmanagement_projects_locations_global_connectivity_tests_rerun_builder, networkmanagement_projects_locations_global_connectivity_tests_rerun_task,
    networkmanagement_projects_locations_global_connectivity_tests_set_iam_policy_builder, networkmanagement_projects_locations_global_connectivity_tests_set_iam_policy_task,
    networkmanagement_projects_locations_global_connectivity_tests_test_iam_permissions_builder, networkmanagement_projects_locations_global_connectivity_tests_test_iam_permissions_task,
    networkmanagement_projects_locations_global_operations_cancel_builder, networkmanagement_projects_locations_global_operations_cancel_task,
    networkmanagement_projects_locations_global_operations_delete_builder, networkmanagement_projects_locations_global_operations_delete_task,
    networkmanagement_projects_locations_global_operations_get_builder, networkmanagement_projects_locations_global_operations_get_task,
    networkmanagement_projects_locations_global_operations_list_builder, networkmanagement_projects_locations_global_operations_list_task,
    networkmanagement_projects_locations_vpc_flow_logs_configs_create_builder, networkmanagement_projects_locations_vpc_flow_logs_configs_create_task,
    networkmanagement_projects_locations_vpc_flow_logs_configs_delete_builder, networkmanagement_projects_locations_vpc_flow_logs_configs_delete_task,
    networkmanagement_projects_locations_vpc_flow_logs_configs_get_builder, networkmanagement_projects_locations_vpc_flow_logs_configs_get_task,
    networkmanagement_projects_locations_vpc_flow_logs_configs_list_builder, networkmanagement_projects_locations_vpc_flow_logs_configs_list_task,
    networkmanagement_projects_locations_vpc_flow_logs_configs_patch_builder, networkmanagement_projects_locations_vpc_flow_logs_configs_patch_task,
    networkmanagement_projects_locations_vpc_flow_logs_configs_query_org_vpc_flow_logs_configs_builder, networkmanagement_projects_locations_vpc_flow_logs_configs_query_org_vpc_flow_logs_configs_task,
    networkmanagement_projects_locations_vpc_flow_logs_configs_show_effective_flow_logs_configs_builder, networkmanagement_projects_locations_vpc_flow_logs_configs_show_effective_flow_logs_configs_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::networkmanagement::ConnectivityTest;
use crate::providers::gcp::clients::networkmanagement::Empty;
use crate::providers::gcp::clients::networkmanagement::ListConnectivityTestsResponse;
use crate::providers::gcp::clients::networkmanagement::ListLocationsResponse;
use crate::providers::gcp::clients::networkmanagement::ListOperationsResponse;
use crate::providers::gcp::clients::networkmanagement::ListVpcFlowLogsConfigsResponse;
use crate::providers::gcp::clients::networkmanagement::Location;
use crate::providers::gcp::clients::networkmanagement::Operation;
use crate::providers::gcp::clients::networkmanagement::Policy;
use crate::providers::gcp::clients::networkmanagement::QueryOrgVpcFlowLogsConfigsResponse;
use crate::providers::gcp::clients::networkmanagement::ShowEffectiveFlowLogsConfigsResponse;
use crate::providers::gcp::clients::networkmanagement::TestIamPermissionsResponse;
use crate::providers::gcp::clients::networkmanagement::VpcFlowLogsConfig;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementOrganizationsLocationsGetArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementOrganizationsLocationsGlobalOperationsCancelArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementOrganizationsLocationsGlobalOperationsDeleteArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementOrganizationsLocationsGlobalOperationsGetArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementOrganizationsLocationsGlobalOperationsListArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementOrganizationsLocationsListArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementOrganizationsLocationsVpcFlowLogsConfigsCreateArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementOrganizationsLocationsVpcFlowLogsConfigsDeleteArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementOrganizationsLocationsVpcFlowLogsConfigsGetArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementOrganizationsLocationsVpcFlowLogsConfigsListArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementOrganizationsLocationsVpcFlowLogsConfigsPatchArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementProjectsLocationsGetArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementProjectsLocationsGlobalConnectivityTestsCreateArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementProjectsLocationsGlobalConnectivityTestsDeleteArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementProjectsLocationsGlobalConnectivityTestsGetArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementProjectsLocationsGlobalConnectivityTestsGetIamPolicyArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementProjectsLocationsGlobalConnectivityTestsListArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementProjectsLocationsGlobalConnectivityTestsPatchArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementProjectsLocationsGlobalConnectivityTestsRerunArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementProjectsLocationsGlobalConnectivityTestsSetIamPolicyArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementProjectsLocationsGlobalConnectivityTestsTestIamPermissionsArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementProjectsLocationsGlobalOperationsCancelArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementProjectsLocationsGlobalOperationsDeleteArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementProjectsLocationsGlobalOperationsGetArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementProjectsLocationsGlobalOperationsListArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementProjectsLocationsListArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementProjectsLocationsVpcFlowLogsConfigsCreateArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementProjectsLocationsVpcFlowLogsConfigsDeleteArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementProjectsLocationsVpcFlowLogsConfigsGetArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementProjectsLocationsVpcFlowLogsConfigsListArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementProjectsLocationsVpcFlowLogsConfigsPatchArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementProjectsLocationsVpcFlowLogsConfigsQueryOrgVpcFlowLogsConfigsArgs;
use crate::providers::gcp::clients::networkmanagement::NetworkmanagementProjectsLocationsVpcFlowLogsConfigsShowEffectiveFlowLogsConfigsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// NetworkmanagementProvider with automatic state tracking.
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
/// let provider = NetworkmanagementProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct NetworkmanagementProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> NetworkmanagementProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new NetworkmanagementProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new NetworkmanagementProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Networkmanagement organizations locations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Location result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkmanagement_organizations_locations_get(
        &self,
        args: &NetworkmanagementOrganizationsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_organizations_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_organizations_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement organizations locations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLocationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkmanagement_organizations_locations_list(
        &self,
        args: &NetworkmanagementOrganizationsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_organizations_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_organizations_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement organizations locations global operations cancel.
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
    pub fn networkmanagement_organizations_locations_global_operations_cancel(
        &self,
        args: &NetworkmanagementOrganizationsLocationsGlobalOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_organizations_locations_global_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_organizations_locations_global_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement organizations locations global operations delete.
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
    pub fn networkmanagement_organizations_locations_global_operations_delete(
        &self,
        args: &NetworkmanagementOrganizationsLocationsGlobalOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_organizations_locations_global_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_organizations_locations_global_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement organizations locations global operations get.
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
    pub fn networkmanagement_organizations_locations_global_operations_get(
        &self,
        args: &NetworkmanagementOrganizationsLocationsGlobalOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_organizations_locations_global_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_organizations_locations_global_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement organizations locations global operations list.
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
    pub fn networkmanagement_organizations_locations_global_operations_list(
        &self,
        args: &NetworkmanagementOrganizationsLocationsGlobalOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_organizations_locations_global_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_organizations_locations_global_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement organizations locations vpc flow logs configs create.
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
    pub fn networkmanagement_organizations_locations_vpc_flow_logs_configs_create(
        &self,
        args: &NetworkmanagementOrganizationsLocationsVpcFlowLogsConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_organizations_locations_vpc_flow_logs_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.vpcFlowLogsConfigId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_organizations_locations_vpc_flow_logs_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement organizations locations vpc flow logs configs delete.
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
    pub fn networkmanagement_organizations_locations_vpc_flow_logs_configs_delete(
        &self,
        args: &NetworkmanagementOrganizationsLocationsVpcFlowLogsConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_organizations_locations_vpc_flow_logs_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_organizations_locations_vpc_flow_logs_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement organizations locations vpc flow logs configs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VpcFlowLogsConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkmanagement_organizations_locations_vpc_flow_logs_configs_get(
        &self,
        args: &NetworkmanagementOrganizationsLocationsVpcFlowLogsConfigsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VpcFlowLogsConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_organizations_locations_vpc_flow_logs_configs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_organizations_locations_vpc_flow_logs_configs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement organizations locations vpc flow logs configs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListVpcFlowLogsConfigsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkmanagement_organizations_locations_vpc_flow_logs_configs_list(
        &self,
        args: &NetworkmanagementOrganizationsLocationsVpcFlowLogsConfigsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListVpcFlowLogsConfigsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_organizations_locations_vpc_flow_logs_configs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_organizations_locations_vpc_flow_logs_configs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement organizations locations vpc flow logs configs patch.
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
    pub fn networkmanagement_organizations_locations_vpc_flow_logs_configs_patch(
        &self,
        args: &NetworkmanagementOrganizationsLocationsVpcFlowLogsConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_organizations_locations_vpc_flow_logs_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_organizations_locations_vpc_flow_logs_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement projects locations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Location result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkmanagement_projects_locations_get(
        &self,
        args: &NetworkmanagementProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Location, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement projects locations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListLocationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkmanagement_projects_locations_list(
        &self,
        args: &NetworkmanagementProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement projects locations global connectivity tests create.
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
    pub fn networkmanagement_projects_locations_global_connectivity_tests_create(
        &self,
        args: &NetworkmanagementProjectsLocationsGlobalConnectivityTestsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_projects_locations_global_connectivity_tests_create_builder(
            &self.http_client,
            &args.parent,
            &args.testId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_projects_locations_global_connectivity_tests_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement projects locations global connectivity tests delete.
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
    pub fn networkmanagement_projects_locations_global_connectivity_tests_delete(
        &self,
        args: &NetworkmanagementProjectsLocationsGlobalConnectivityTestsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_projects_locations_global_connectivity_tests_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_projects_locations_global_connectivity_tests_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement projects locations global connectivity tests get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ConnectivityTest result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkmanagement_projects_locations_global_connectivity_tests_get(
        &self,
        args: &NetworkmanagementProjectsLocationsGlobalConnectivityTestsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ConnectivityTest, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_projects_locations_global_connectivity_tests_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_projects_locations_global_connectivity_tests_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement projects locations global connectivity tests get iam policy.
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
    pub fn networkmanagement_projects_locations_global_connectivity_tests_get_iam_policy(
        &self,
        args: &NetworkmanagementProjectsLocationsGlobalConnectivityTestsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_projects_locations_global_connectivity_tests_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_projects_locations_global_connectivity_tests_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement projects locations global connectivity tests list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListConnectivityTestsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkmanagement_projects_locations_global_connectivity_tests_list(
        &self,
        args: &NetworkmanagementProjectsLocationsGlobalConnectivityTestsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListConnectivityTestsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_projects_locations_global_connectivity_tests_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_projects_locations_global_connectivity_tests_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement projects locations global connectivity tests patch.
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
    pub fn networkmanagement_projects_locations_global_connectivity_tests_patch(
        &self,
        args: &NetworkmanagementProjectsLocationsGlobalConnectivityTestsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_projects_locations_global_connectivity_tests_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_projects_locations_global_connectivity_tests_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement projects locations global connectivity tests rerun.
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
    pub fn networkmanagement_projects_locations_global_connectivity_tests_rerun(
        &self,
        args: &NetworkmanagementProjectsLocationsGlobalConnectivityTestsRerunArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_projects_locations_global_connectivity_tests_rerun_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_projects_locations_global_connectivity_tests_rerun_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement projects locations global connectivity tests set iam policy.
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
    pub fn networkmanagement_projects_locations_global_connectivity_tests_set_iam_policy(
        &self,
        args: &NetworkmanagementProjectsLocationsGlobalConnectivityTestsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_projects_locations_global_connectivity_tests_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_projects_locations_global_connectivity_tests_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement projects locations global connectivity tests test iam permissions.
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
    pub fn networkmanagement_projects_locations_global_connectivity_tests_test_iam_permissions(
        &self,
        args: &NetworkmanagementProjectsLocationsGlobalConnectivityTestsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_projects_locations_global_connectivity_tests_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_projects_locations_global_connectivity_tests_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement projects locations global operations cancel.
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
    pub fn networkmanagement_projects_locations_global_operations_cancel(
        &self,
        args: &NetworkmanagementProjectsLocationsGlobalOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_projects_locations_global_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_projects_locations_global_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement projects locations global operations delete.
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
    pub fn networkmanagement_projects_locations_global_operations_delete(
        &self,
        args: &NetworkmanagementProjectsLocationsGlobalOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_projects_locations_global_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_projects_locations_global_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement projects locations global operations get.
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
    pub fn networkmanagement_projects_locations_global_operations_get(
        &self,
        args: &NetworkmanagementProjectsLocationsGlobalOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_projects_locations_global_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_projects_locations_global_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement projects locations global operations list.
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
    pub fn networkmanagement_projects_locations_global_operations_list(
        &self,
        args: &NetworkmanagementProjectsLocationsGlobalOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_projects_locations_global_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_projects_locations_global_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement projects locations vpc flow logs configs create.
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
    pub fn networkmanagement_projects_locations_vpc_flow_logs_configs_create(
        &self,
        args: &NetworkmanagementProjectsLocationsVpcFlowLogsConfigsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_projects_locations_vpc_flow_logs_configs_create_builder(
            &self.http_client,
            &args.parent,
            &args.vpcFlowLogsConfigId,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_projects_locations_vpc_flow_logs_configs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement projects locations vpc flow logs configs delete.
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
    pub fn networkmanagement_projects_locations_vpc_flow_logs_configs_delete(
        &self,
        args: &NetworkmanagementProjectsLocationsVpcFlowLogsConfigsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_projects_locations_vpc_flow_logs_configs_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_projects_locations_vpc_flow_logs_configs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement projects locations vpc flow logs configs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VpcFlowLogsConfig result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkmanagement_projects_locations_vpc_flow_logs_configs_get(
        &self,
        args: &NetworkmanagementProjectsLocationsVpcFlowLogsConfigsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VpcFlowLogsConfig, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_projects_locations_vpc_flow_logs_configs_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_projects_locations_vpc_flow_logs_configs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement projects locations vpc flow logs configs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListVpcFlowLogsConfigsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkmanagement_projects_locations_vpc_flow_logs_configs_list(
        &self,
        args: &NetworkmanagementProjectsLocationsVpcFlowLogsConfigsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListVpcFlowLogsConfigsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_projects_locations_vpc_flow_logs_configs_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_projects_locations_vpc_flow_logs_configs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement projects locations vpc flow logs configs patch.
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
    pub fn networkmanagement_projects_locations_vpc_flow_logs_configs_patch(
        &self,
        args: &NetworkmanagementProjectsLocationsVpcFlowLogsConfigsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_projects_locations_vpc_flow_logs_configs_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_projects_locations_vpc_flow_logs_configs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement projects locations vpc flow logs configs query org vpc flow logs configs.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the QueryOrgVpcFlowLogsConfigsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkmanagement_projects_locations_vpc_flow_logs_configs_query_org_vpc_flow_logs_configs(
        &self,
        args: &NetworkmanagementProjectsLocationsVpcFlowLogsConfigsQueryOrgVpcFlowLogsConfigsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<QueryOrgVpcFlowLogsConfigsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_projects_locations_vpc_flow_logs_configs_query_org_vpc_flow_logs_configs_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_projects_locations_vpc_flow_logs_configs_query_org_vpc_flow_logs_configs_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Networkmanagement projects locations vpc flow logs configs show effective flow logs configs.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ShowEffectiveFlowLogsConfigsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn networkmanagement_projects_locations_vpc_flow_logs_configs_show_effective_flow_logs_configs(
        &self,
        args: &NetworkmanagementProjectsLocationsVpcFlowLogsConfigsShowEffectiveFlowLogsConfigsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ShowEffectiveFlowLogsConfigsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = networkmanagement_projects_locations_vpc_flow_logs_configs_show_effective_flow_logs_configs_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = networkmanagement_projects_locations_vpc_flow_logs_configs_show_effective_flow_logs_configs_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
