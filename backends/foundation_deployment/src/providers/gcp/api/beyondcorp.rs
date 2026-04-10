//! BeyondcorpProvider - State-aware beyondcorp API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       beyondcorp API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::beyondcorp::{
    beyondcorp_organizations_locations_operations_cancel_builder, beyondcorp_organizations_locations_operations_cancel_task,
    beyondcorp_organizations_locations_operations_delete_builder, beyondcorp_organizations_locations_operations_delete_task,
    beyondcorp_organizations_locations_operations_get_builder, beyondcorp_organizations_locations_operations_get_task,
    beyondcorp_organizations_locations_operations_list_builder, beyondcorp_organizations_locations_operations_list_task,
    beyondcorp_projects_locations_get_builder, beyondcorp_projects_locations_get_task,
    beyondcorp_projects_locations_list_builder, beyondcorp_projects_locations_list_task,
    beyondcorp_projects_locations_app_connections_create_builder, beyondcorp_projects_locations_app_connections_create_task,
    beyondcorp_projects_locations_app_connections_delete_builder, beyondcorp_projects_locations_app_connections_delete_task,
    beyondcorp_projects_locations_app_connections_get_builder, beyondcorp_projects_locations_app_connections_get_task,
    beyondcorp_projects_locations_app_connections_get_iam_policy_builder, beyondcorp_projects_locations_app_connections_get_iam_policy_task,
    beyondcorp_projects_locations_app_connections_list_builder, beyondcorp_projects_locations_app_connections_list_task,
    beyondcorp_projects_locations_app_connections_patch_builder, beyondcorp_projects_locations_app_connections_patch_task,
    beyondcorp_projects_locations_app_connections_resolve_builder, beyondcorp_projects_locations_app_connections_resolve_task,
    beyondcorp_projects_locations_app_connections_set_iam_policy_builder, beyondcorp_projects_locations_app_connections_set_iam_policy_task,
    beyondcorp_projects_locations_app_connections_test_iam_permissions_builder, beyondcorp_projects_locations_app_connections_test_iam_permissions_task,
    beyondcorp_projects_locations_app_connectors_create_builder, beyondcorp_projects_locations_app_connectors_create_task,
    beyondcorp_projects_locations_app_connectors_delete_builder, beyondcorp_projects_locations_app_connectors_delete_task,
    beyondcorp_projects_locations_app_connectors_get_builder, beyondcorp_projects_locations_app_connectors_get_task,
    beyondcorp_projects_locations_app_connectors_get_iam_policy_builder, beyondcorp_projects_locations_app_connectors_get_iam_policy_task,
    beyondcorp_projects_locations_app_connectors_list_builder, beyondcorp_projects_locations_app_connectors_list_task,
    beyondcorp_projects_locations_app_connectors_patch_builder, beyondcorp_projects_locations_app_connectors_patch_task,
    beyondcorp_projects_locations_app_connectors_report_status_builder, beyondcorp_projects_locations_app_connectors_report_status_task,
    beyondcorp_projects_locations_app_connectors_resolve_instance_config_builder, beyondcorp_projects_locations_app_connectors_resolve_instance_config_task,
    beyondcorp_projects_locations_app_connectors_set_iam_policy_builder, beyondcorp_projects_locations_app_connectors_set_iam_policy_task,
    beyondcorp_projects_locations_app_connectors_test_iam_permissions_builder, beyondcorp_projects_locations_app_connectors_test_iam_permissions_task,
    beyondcorp_projects_locations_app_gateways_create_builder, beyondcorp_projects_locations_app_gateways_create_task,
    beyondcorp_projects_locations_app_gateways_delete_builder, beyondcorp_projects_locations_app_gateways_delete_task,
    beyondcorp_projects_locations_app_gateways_get_builder, beyondcorp_projects_locations_app_gateways_get_task,
    beyondcorp_projects_locations_app_gateways_get_iam_policy_builder, beyondcorp_projects_locations_app_gateways_get_iam_policy_task,
    beyondcorp_projects_locations_app_gateways_list_builder, beyondcorp_projects_locations_app_gateways_list_task,
    beyondcorp_projects_locations_app_gateways_set_iam_policy_builder, beyondcorp_projects_locations_app_gateways_set_iam_policy_task,
    beyondcorp_projects_locations_app_gateways_test_iam_permissions_builder, beyondcorp_projects_locations_app_gateways_test_iam_permissions_task,
    beyondcorp_projects_locations_operations_cancel_builder, beyondcorp_projects_locations_operations_cancel_task,
    beyondcorp_projects_locations_operations_delete_builder, beyondcorp_projects_locations_operations_delete_task,
    beyondcorp_projects_locations_operations_get_builder, beyondcorp_projects_locations_operations_get_task,
    beyondcorp_projects_locations_operations_list_builder, beyondcorp_projects_locations_operations_list_task,
    beyondcorp_projects_locations_security_gateways_create_builder, beyondcorp_projects_locations_security_gateways_create_task,
    beyondcorp_projects_locations_security_gateways_delete_builder, beyondcorp_projects_locations_security_gateways_delete_task,
    beyondcorp_projects_locations_security_gateways_get_builder, beyondcorp_projects_locations_security_gateways_get_task,
    beyondcorp_projects_locations_security_gateways_get_iam_policy_builder, beyondcorp_projects_locations_security_gateways_get_iam_policy_task,
    beyondcorp_projects_locations_security_gateways_list_builder, beyondcorp_projects_locations_security_gateways_list_task,
    beyondcorp_projects_locations_security_gateways_patch_builder, beyondcorp_projects_locations_security_gateways_patch_task,
    beyondcorp_projects_locations_security_gateways_set_iam_policy_builder, beyondcorp_projects_locations_security_gateways_set_iam_policy_task,
    beyondcorp_projects_locations_security_gateways_test_iam_permissions_builder, beyondcorp_projects_locations_security_gateways_test_iam_permissions_task,
    beyondcorp_projects_locations_security_gateways_applications_create_builder, beyondcorp_projects_locations_security_gateways_applications_create_task,
    beyondcorp_projects_locations_security_gateways_applications_delete_builder, beyondcorp_projects_locations_security_gateways_applications_delete_task,
    beyondcorp_projects_locations_security_gateways_applications_get_builder, beyondcorp_projects_locations_security_gateways_applications_get_task,
    beyondcorp_projects_locations_security_gateways_applications_get_iam_policy_builder, beyondcorp_projects_locations_security_gateways_applications_get_iam_policy_task,
    beyondcorp_projects_locations_security_gateways_applications_list_builder, beyondcorp_projects_locations_security_gateways_applications_list_task,
    beyondcorp_projects_locations_security_gateways_applications_patch_builder, beyondcorp_projects_locations_security_gateways_applications_patch_task,
    beyondcorp_projects_locations_security_gateways_applications_set_iam_policy_builder, beyondcorp_projects_locations_security_gateways_applications_set_iam_policy_task,
    beyondcorp_projects_locations_security_gateways_applications_test_iam_permissions_builder, beyondcorp_projects_locations_security_gateways_applications_test_iam_permissions_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::beyondcorp::AppGateway;
use crate::providers::gcp::clients::beyondcorp::Empty;
use crate::providers::gcp::clients::beyondcorp::GoogleCloudBeyondcorpAppconnectionsV1AppConnection;
use crate::providers::gcp::clients::beyondcorp::GoogleCloudBeyondcorpAppconnectionsV1ListAppConnectionsResponse;
use crate::providers::gcp::clients::beyondcorp::GoogleCloudBeyondcorpAppconnectionsV1ResolveAppConnectionsResponse;
use crate::providers::gcp::clients::beyondcorp::GoogleCloudBeyondcorpAppconnectorsV1AppConnector;
use crate::providers::gcp::clients::beyondcorp::GoogleCloudBeyondcorpAppconnectorsV1ListAppConnectorsResponse;
use crate::providers::gcp::clients::beyondcorp::GoogleCloudBeyondcorpAppconnectorsV1ResolveInstanceConfigResponse;
use crate::providers::gcp::clients::beyondcorp::GoogleCloudBeyondcorpSecuritygatewaysV1Application;
use crate::providers::gcp::clients::beyondcorp::GoogleCloudBeyondcorpSecuritygatewaysV1ListApplicationsResponse;
use crate::providers::gcp::clients::beyondcorp::GoogleCloudBeyondcorpSecuritygatewaysV1ListSecurityGatewaysResponse;
use crate::providers::gcp::clients::beyondcorp::GoogleCloudBeyondcorpSecuritygatewaysV1SecurityGateway;
use crate::providers::gcp::clients::beyondcorp::GoogleCloudLocationListLocationsResponse;
use crate::providers::gcp::clients::beyondcorp::GoogleCloudLocationLocation;
use crate::providers::gcp::clients::beyondcorp::GoogleIamV1Policy;
use crate::providers::gcp::clients::beyondcorp::GoogleIamV1TestIamPermissionsResponse;
use crate::providers::gcp::clients::beyondcorp::GoogleLongrunningListOperationsResponse;
use crate::providers::gcp::clients::beyondcorp::GoogleLongrunningOperation;
use crate::providers::gcp::clients::beyondcorp::ListAppGatewaysResponse;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpOrganizationsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpOrganizationsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpOrganizationsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpOrganizationsLocationsOperationsListArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppConnectionsCreateArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppConnectionsDeleteArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppConnectionsGetArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppConnectionsGetIamPolicyArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppConnectionsListArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppConnectionsPatchArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppConnectionsResolveArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppConnectionsSetIamPolicyArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppConnectionsTestIamPermissionsArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppConnectorsCreateArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppConnectorsDeleteArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppConnectorsGetArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppConnectorsGetIamPolicyArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppConnectorsListArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppConnectorsPatchArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppConnectorsReportStatusArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppConnectorsResolveInstanceConfigArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppConnectorsSetIamPolicyArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppConnectorsTestIamPermissionsArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppGatewaysCreateArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppGatewaysDeleteArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppGatewaysGetArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppGatewaysGetIamPolicyArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppGatewaysListArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppGatewaysSetIamPolicyArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsAppGatewaysTestIamPermissionsArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsGetArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsListArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsOperationsCancelArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsOperationsDeleteArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsOperationsGetArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsOperationsListArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsSecurityGatewaysApplicationsCreateArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsSecurityGatewaysApplicationsDeleteArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsSecurityGatewaysApplicationsGetArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsSecurityGatewaysApplicationsGetIamPolicyArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsSecurityGatewaysApplicationsListArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsSecurityGatewaysApplicationsPatchArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsSecurityGatewaysApplicationsSetIamPolicyArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsSecurityGatewaysApplicationsTestIamPermissionsArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsSecurityGatewaysCreateArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsSecurityGatewaysDeleteArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsSecurityGatewaysGetArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsSecurityGatewaysGetIamPolicyArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsSecurityGatewaysListArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsSecurityGatewaysPatchArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsSecurityGatewaysSetIamPolicyArgs;
use crate::providers::gcp::clients::beyondcorp::BeyondcorpProjectsLocationsSecurityGatewaysTestIamPermissionsArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// BeyondcorpProvider with automatic state tracking.
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
/// let provider = BeyondcorpProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct BeyondcorpProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> BeyondcorpProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new BeyondcorpProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Beyondcorp organizations locations operations cancel.
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
    pub fn beyondcorp_organizations_locations_operations_cancel(
        &self,
        args: &BeyondcorpOrganizationsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_organizations_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_organizations_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp organizations locations operations delete.
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
    pub fn beyondcorp_organizations_locations_operations_delete(
        &self,
        args: &BeyondcorpOrganizationsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_organizations_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_organizations_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp organizations locations operations get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_organizations_locations_operations_get(
        &self,
        args: &BeyondcorpOrganizationsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_organizations_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_organizations_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp organizations locations operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningListOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_organizations_locations_operations_list(
        &self,
        args: &BeyondcorpOrganizationsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_organizations_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_organizations_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudLocationLocation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_get(
        &self,
        args: &BeyondcorpProjectsLocationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudLocationLocation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudLocationListLocationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_list(
        &self,
        args: &BeyondcorpProjectsLocationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudLocationListLocationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_list_builder(
            &self.http_client,
            &args.name,
            &args.extraLocationTypes,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app connections create.
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
    pub fn beyondcorp_projects_locations_app_connections_create(
        &self,
        args: &BeyondcorpProjectsLocationsAppConnectionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_connections_create_builder(
            &self.http_client,
            &args.parent,
            &args.appConnectionId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_connections_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app connections delete.
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
    pub fn beyondcorp_projects_locations_app_connections_delete(
        &self,
        args: &BeyondcorpProjectsLocationsAppConnectionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_connections_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_connections_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app connections get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudBeyondcorpAppconnectionsV1AppConnection result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_app_connections_get(
        &self,
        args: &BeyondcorpProjectsLocationsAppConnectionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudBeyondcorpAppconnectionsV1AppConnection, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_connections_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_connections_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app connections get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_app_connections_get_iam_policy(
        &self,
        args: &BeyondcorpProjectsLocationsAppConnectionsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_connections_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_connections_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app connections list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudBeyondcorpAppconnectionsV1ListAppConnectionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_app_connections_list(
        &self,
        args: &BeyondcorpProjectsLocationsAppConnectionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudBeyondcorpAppconnectionsV1ListAppConnectionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_connections_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_connections_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app connections patch.
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
    pub fn beyondcorp_projects_locations_app_connections_patch(
        &self,
        args: &BeyondcorpProjectsLocationsAppConnectionsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_connections_patch_builder(
            &self.http_client,
            &args.name,
            &args.allowMissing,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_connections_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app connections resolve.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudBeyondcorpAppconnectionsV1ResolveAppConnectionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_app_connections_resolve(
        &self,
        args: &BeyondcorpProjectsLocationsAppConnectionsResolveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudBeyondcorpAppconnectionsV1ResolveAppConnectionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_connections_resolve_builder(
            &self.http_client,
            &args.parent,
            &args.appConnectorId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_connections_resolve_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app connections set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn beyondcorp_projects_locations_app_connections_set_iam_policy(
        &self,
        args: &BeyondcorpProjectsLocationsAppConnectionsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_connections_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_connections_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app connections test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_app_connections_test_iam_permissions(
        &self,
        args: &BeyondcorpProjectsLocationsAppConnectionsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_connections_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_connections_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app connectors create.
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
    pub fn beyondcorp_projects_locations_app_connectors_create(
        &self,
        args: &BeyondcorpProjectsLocationsAppConnectorsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_connectors_create_builder(
            &self.http_client,
            &args.parent,
            &args.appConnectorId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_connectors_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app connectors delete.
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
    pub fn beyondcorp_projects_locations_app_connectors_delete(
        &self,
        args: &BeyondcorpProjectsLocationsAppConnectorsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_connectors_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_connectors_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app connectors get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudBeyondcorpAppconnectorsV1AppConnector result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_app_connectors_get(
        &self,
        args: &BeyondcorpProjectsLocationsAppConnectorsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudBeyondcorpAppconnectorsV1AppConnector, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_connectors_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_connectors_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app connectors get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_app_connectors_get_iam_policy(
        &self,
        args: &BeyondcorpProjectsLocationsAppConnectorsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_connectors_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_connectors_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app connectors list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudBeyondcorpAppconnectorsV1ListAppConnectorsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_app_connectors_list(
        &self,
        args: &BeyondcorpProjectsLocationsAppConnectorsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudBeyondcorpAppconnectorsV1ListAppConnectorsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_connectors_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_connectors_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app connectors patch.
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
    pub fn beyondcorp_projects_locations_app_connectors_patch(
        &self,
        args: &BeyondcorpProjectsLocationsAppConnectorsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_connectors_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_connectors_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app connectors report status.
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
    pub fn beyondcorp_projects_locations_app_connectors_report_status(
        &self,
        args: &BeyondcorpProjectsLocationsAppConnectorsReportStatusArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_connectors_report_status_builder(
            &self.http_client,
            &args.appConnector,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_connectors_report_status_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app connectors resolve instance config.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudBeyondcorpAppconnectorsV1ResolveInstanceConfigResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_app_connectors_resolve_instance_config(
        &self,
        args: &BeyondcorpProjectsLocationsAppConnectorsResolveInstanceConfigArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudBeyondcorpAppconnectorsV1ResolveInstanceConfigResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_connectors_resolve_instance_config_builder(
            &self.http_client,
            &args.appConnector,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_connectors_resolve_instance_config_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app connectors set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn beyondcorp_projects_locations_app_connectors_set_iam_policy(
        &self,
        args: &BeyondcorpProjectsLocationsAppConnectorsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_connectors_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_connectors_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app connectors test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_app_connectors_test_iam_permissions(
        &self,
        args: &BeyondcorpProjectsLocationsAppConnectorsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_connectors_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_connectors_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app gateways create.
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
    pub fn beyondcorp_projects_locations_app_gateways_create(
        &self,
        args: &BeyondcorpProjectsLocationsAppGatewaysCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_gateways_create_builder(
            &self.http_client,
            &args.parent,
            &args.appGatewayId,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_gateways_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app gateways delete.
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
    pub fn beyondcorp_projects_locations_app_gateways_delete(
        &self,
        args: &BeyondcorpProjectsLocationsAppGatewaysDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_gateways_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_gateways_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app gateways get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AppGateway result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_app_gateways_get(
        &self,
        args: &BeyondcorpProjectsLocationsAppGatewaysGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AppGateway, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_gateways_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_gateways_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app gateways get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_app_gateways_get_iam_policy(
        &self,
        args: &BeyondcorpProjectsLocationsAppGatewaysGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_gateways_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_gateways_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app gateways list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAppGatewaysResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_app_gateways_list(
        &self,
        args: &BeyondcorpProjectsLocationsAppGatewaysListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAppGatewaysResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_gateways_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_gateways_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app gateways set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn beyondcorp_projects_locations_app_gateways_set_iam_policy(
        &self,
        args: &BeyondcorpProjectsLocationsAppGatewaysSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_gateways_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_gateways_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations app gateways test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_app_gateways_test_iam_permissions(
        &self,
        args: &BeyondcorpProjectsLocationsAppGatewaysTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_app_gateways_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_app_gateways_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations operations cancel.
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
    pub fn beyondcorp_projects_locations_operations_cancel(
        &self,
        args: &BeyondcorpProjectsLocationsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations operations delete.
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
    pub fn beyondcorp_projects_locations_operations_delete(
        &self,
        args: &BeyondcorpProjectsLocationsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations operations get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_operations_get(
        &self,
        args: &BeyondcorpProjectsLocationsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningListOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_operations_list(
        &self,
        args: &BeyondcorpProjectsLocationsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations security gateways create.
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
    pub fn beyondcorp_projects_locations_security_gateways_create(
        &self,
        args: &BeyondcorpProjectsLocationsSecurityGatewaysCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_security_gateways_create_builder(
            &self.http_client,
            &args.parent,
            &args.requestId,
            &args.securityGatewayId,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_security_gateways_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations security gateways delete.
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
    pub fn beyondcorp_projects_locations_security_gateways_delete(
        &self,
        args: &BeyondcorpProjectsLocationsSecurityGatewaysDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_security_gateways_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_security_gateways_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations security gateways get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudBeyondcorpSecuritygatewaysV1SecurityGateway result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_security_gateways_get(
        &self,
        args: &BeyondcorpProjectsLocationsSecurityGatewaysGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudBeyondcorpSecuritygatewaysV1SecurityGateway, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_security_gateways_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_security_gateways_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations security gateways get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_security_gateways_get_iam_policy(
        &self,
        args: &BeyondcorpProjectsLocationsSecurityGatewaysGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_security_gateways_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_security_gateways_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations security gateways list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudBeyondcorpSecuritygatewaysV1ListSecurityGatewaysResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_security_gateways_list(
        &self,
        args: &BeyondcorpProjectsLocationsSecurityGatewaysListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudBeyondcorpSecuritygatewaysV1ListSecurityGatewaysResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_security_gateways_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_security_gateways_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations security gateways patch.
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
    pub fn beyondcorp_projects_locations_security_gateways_patch(
        &self,
        args: &BeyondcorpProjectsLocationsSecurityGatewaysPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_security_gateways_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_security_gateways_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations security gateways set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn beyondcorp_projects_locations_security_gateways_set_iam_policy(
        &self,
        args: &BeyondcorpProjectsLocationsSecurityGatewaysSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_security_gateways_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_security_gateways_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations security gateways test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_security_gateways_test_iam_permissions(
        &self,
        args: &BeyondcorpProjectsLocationsSecurityGatewaysTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_security_gateways_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_security_gateways_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations security gateways applications create.
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
    pub fn beyondcorp_projects_locations_security_gateways_applications_create(
        &self,
        args: &BeyondcorpProjectsLocationsSecurityGatewaysApplicationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_security_gateways_applications_create_builder(
            &self.http_client,
            &args.parent,
            &args.applicationId,
            &args.requestId,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_security_gateways_applications_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations security gateways applications delete.
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
    pub fn beyondcorp_projects_locations_security_gateways_applications_delete(
        &self,
        args: &BeyondcorpProjectsLocationsSecurityGatewaysApplicationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_security_gateways_applications_delete_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.validateOnly,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_security_gateways_applications_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations security gateways applications get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudBeyondcorpSecuritygatewaysV1Application result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_security_gateways_applications_get(
        &self,
        args: &BeyondcorpProjectsLocationsSecurityGatewaysApplicationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudBeyondcorpSecuritygatewaysV1Application, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_security_gateways_applications_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_security_gateways_applications_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations security gateways applications get iam policy.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_security_gateways_applications_get_iam_policy(
        &self,
        args: &BeyondcorpProjectsLocationsSecurityGatewaysApplicationsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_security_gateways_applications_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options.requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_security_gateways_applications_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations security gateways applications list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudBeyondcorpSecuritygatewaysV1ListApplicationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_security_gateways_applications_list(
        &self,
        args: &BeyondcorpProjectsLocationsSecurityGatewaysApplicationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudBeyondcorpSecuritygatewaysV1ListApplicationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_security_gateways_applications_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.orderBy,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_security_gateways_applications_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations security gateways applications patch.
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
    pub fn beyondcorp_projects_locations_security_gateways_applications_patch(
        &self,
        args: &BeyondcorpProjectsLocationsSecurityGatewaysApplicationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_security_gateways_applications_patch_builder(
            &self.http_client,
            &args.name,
            &args.requestId,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_security_gateways_applications_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations security gateways applications set iam policy.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1Policy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn beyondcorp_projects_locations_security_gateways_applications_set_iam_policy(
        &self,
        args: &BeyondcorpProjectsLocationsSecurityGatewaysApplicationsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_security_gateways_applications_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_security_gateways_applications_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Beyondcorp projects locations security gateways applications test iam permissions.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleIamV1TestIamPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn beyondcorp_projects_locations_security_gateways_applications_test_iam_permissions(
        &self,
        args: &BeyondcorpProjectsLocationsSecurityGatewaysApplicationsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = beyondcorp_projects_locations_security_gateways_applications_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = beyondcorp_projects_locations_security_gateways_applications_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
