//! ProdTtSasportalProvider - State-aware prod_tt_sasportal API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       prod_tt_sasportal API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::prod_tt_sasportal::{
    prod_tt_sasportal_customers_get_builder, prod_tt_sasportal_customers_get_task,
    prod_tt_sasportal_customers_list_builder, prod_tt_sasportal_customers_list_task,
    prod_tt_sasportal_customers_list_gcp_project_deployments_builder, prod_tt_sasportal_customers_list_gcp_project_deployments_task,
    prod_tt_sasportal_customers_list_legacy_organizations_builder, prod_tt_sasportal_customers_list_legacy_organizations_task,
    prod_tt_sasportal_customers_migrate_organization_builder, prod_tt_sasportal_customers_migrate_organization_task,
    prod_tt_sasportal_customers_patch_builder, prod_tt_sasportal_customers_patch_task,
    prod_tt_sasportal_customers_provision_deployment_builder, prod_tt_sasportal_customers_provision_deployment_task,
    prod_tt_sasportal_customers_setup_sas_analytics_builder, prod_tt_sasportal_customers_setup_sas_analytics_task,
    prod_tt_sasportal_customers_deployments_create_builder, prod_tt_sasportal_customers_deployments_create_task,
    prod_tt_sasportal_customers_deployments_delete_builder, prod_tt_sasportal_customers_deployments_delete_task,
    prod_tt_sasportal_customers_deployments_get_builder, prod_tt_sasportal_customers_deployments_get_task,
    prod_tt_sasportal_customers_deployments_list_builder, prod_tt_sasportal_customers_deployments_list_task,
    prod_tt_sasportal_customers_deployments_move_builder, prod_tt_sasportal_customers_deployments_move_task,
    prod_tt_sasportal_customers_deployments_patch_builder, prod_tt_sasportal_customers_deployments_patch_task,
    prod_tt_sasportal_customers_deployments_devices_create_builder, prod_tt_sasportal_customers_deployments_devices_create_task,
    prod_tt_sasportal_customers_deployments_devices_create_signed_builder, prod_tt_sasportal_customers_deployments_devices_create_signed_task,
    prod_tt_sasportal_customers_deployments_devices_list_builder, prod_tt_sasportal_customers_deployments_devices_list_task,
    prod_tt_sasportal_customers_devices_create_builder, prod_tt_sasportal_customers_devices_create_task,
    prod_tt_sasportal_customers_devices_create_signed_builder, prod_tt_sasportal_customers_devices_create_signed_task,
    prod_tt_sasportal_customers_devices_delete_builder, prod_tt_sasportal_customers_devices_delete_task,
    prod_tt_sasportal_customers_devices_get_builder, prod_tt_sasportal_customers_devices_get_task,
    prod_tt_sasportal_customers_devices_list_builder, prod_tt_sasportal_customers_devices_list_task,
    prod_tt_sasportal_customers_devices_move_builder, prod_tt_sasportal_customers_devices_move_task,
    prod_tt_sasportal_customers_devices_patch_builder, prod_tt_sasportal_customers_devices_patch_task,
    prod_tt_sasportal_customers_devices_sign_device_builder, prod_tt_sasportal_customers_devices_sign_device_task,
    prod_tt_sasportal_customers_devices_update_signed_builder, prod_tt_sasportal_customers_devices_update_signed_task,
    prod_tt_sasportal_customers_nodes_create_builder, prod_tt_sasportal_customers_nodes_create_task,
    prod_tt_sasportal_customers_nodes_delete_builder, prod_tt_sasportal_customers_nodes_delete_task,
    prod_tt_sasportal_customers_nodes_get_builder, prod_tt_sasportal_customers_nodes_get_task,
    prod_tt_sasportal_customers_nodes_list_builder, prod_tt_sasportal_customers_nodes_list_task,
    prod_tt_sasportal_customers_nodes_move_builder, prod_tt_sasportal_customers_nodes_move_task,
    prod_tt_sasportal_customers_nodes_patch_builder, prod_tt_sasportal_customers_nodes_patch_task,
    prod_tt_sasportal_customers_nodes_deployments_create_builder, prod_tt_sasportal_customers_nodes_deployments_create_task,
    prod_tt_sasportal_customers_nodes_deployments_list_builder, prod_tt_sasportal_customers_nodes_deployments_list_task,
    prod_tt_sasportal_customers_nodes_devices_create_builder, prod_tt_sasportal_customers_nodes_devices_create_task,
    prod_tt_sasportal_customers_nodes_devices_create_signed_builder, prod_tt_sasportal_customers_nodes_devices_create_signed_task,
    prod_tt_sasportal_customers_nodes_devices_list_builder, prod_tt_sasportal_customers_nodes_devices_list_task,
    prod_tt_sasportal_customers_nodes_nodes_create_builder, prod_tt_sasportal_customers_nodes_nodes_create_task,
    prod_tt_sasportal_customers_nodes_nodes_list_builder, prod_tt_sasportal_customers_nodes_nodes_list_task,
    prod_tt_sasportal_deployments_get_builder, prod_tt_sasportal_deployments_get_task,
    prod_tt_sasportal_deployments_devices_delete_builder, prod_tt_sasportal_deployments_devices_delete_task,
    prod_tt_sasportal_deployments_devices_get_builder, prod_tt_sasportal_deployments_devices_get_task,
    prod_tt_sasportal_deployments_devices_move_builder, prod_tt_sasportal_deployments_devices_move_task,
    prod_tt_sasportal_deployments_devices_patch_builder, prod_tt_sasportal_deployments_devices_patch_task,
    prod_tt_sasportal_deployments_devices_sign_device_builder, prod_tt_sasportal_deployments_devices_sign_device_task,
    prod_tt_sasportal_deployments_devices_update_signed_builder, prod_tt_sasportal_deployments_devices_update_signed_task,
    prod_tt_sasportal_installer_generate_secret_builder, prod_tt_sasportal_installer_generate_secret_task,
    prod_tt_sasportal_installer_validate_builder, prod_tt_sasportal_installer_validate_task,
    prod_tt_sasportal_nodes_get_builder, prod_tt_sasportal_nodes_get_task,
    prod_tt_sasportal_nodes_deployments_delete_builder, prod_tt_sasportal_nodes_deployments_delete_task,
    prod_tt_sasportal_nodes_deployments_get_builder, prod_tt_sasportal_nodes_deployments_get_task,
    prod_tt_sasportal_nodes_deployments_list_builder, prod_tt_sasportal_nodes_deployments_list_task,
    prod_tt_sasportal_nodes_deployments_move_builder, prod_tt_sasportal_nodes_deployments_move_task,
    prod_tt_sasportal_nodes_deployments_patch_builder, prod_tt_sasportal_nodes_deployments_patch_task,
    prod_tt_sasportal_nodes_deployments_devices_create_builder, prod_tt_sasportal_nodes_deployments_devices_create_task,
    prod_tt_sasportal_nodes_deployments_devices_create_signed_builder, prod_tt_sasportal_nodes_deployments_devices_create_signed_task,
    prod_tt_sasportal_nodes_deployments_devices_list_builder, prod_tt_sasportal_nodes_deployments_devices_list_task,
    prod_tt_sasportal_nodes_devices_create_builder, prod_tt_sasportal_nodes_devices_create_task,
    prod_tt_sasportal_nodes_devices_create_signed_builder, prod_tt_sasportal_nodes_devices_create_signed_task,
    prod_tt_sasportal_nodes_devices_delete_builder, prod_tt_sasportal_nodes_devices_delete_task,
    prod_tt_sasportal_nodes_devices_get_builder, prod_tt_sasportal_nodes_devices_get_task,
    prod_tt_sasportal_nodes_devices_list_builder, prod_tt_sasportal_nodes_devices_list_task,
    prod_tt_sasportal_nodes_devices_move_builder, prod_tt_sasportal_nodes_devices_move_task,
    prod_tt_sasportal_nodes_devices_patch_builder, prod_tt_sasportal_nodes_devices_patch_task,
    prod_tt_sasportal_nodes_devices_sign_device_builder, prod_tt_sasportal_nodes_devices_sign_device_task,
    prod_tt_sasportal_nodes_devices_update_signed_builder, prod_tt_sasportal_nodes_devices_update_signed_task,
    prod_tt_sasportal_nodes_nodes_create_builder, prod_tt_sasportal_nodes_nodes_create_task,
    prod_tt_sasportal_nodes_nodes_delete_builder, prod_tt_sasportal_nodes_nodes_delete_task,
    prod_tt_sasportal_nodes_nodes_get_builder, prod_tt_sasportal_nodes_nodes_get_task,
    prod_tt_sasportal_nodes_nodes_list_builder, prod_tt_sasportal_nodes_nodes_list_task,
    prod_tt_sasportal_nodes_nodes_move_builder, prod_tt_sasportal_nodes_nodes_move_task,
    prod_tt_sasportal_nodes_nodes_patch_builder, prod_tt_sasportal_nodes_nodes_patch_task,
    prod_tt_sasportal_nodes_nodes_deployments_create_builder, prod_tt_sasportal_nodes_nodes_deployments_create_task,
    prod_tt_sasportal_nodes_nodes_deployments_list_builder, prod_tt_sasportal_nodes_nodes_deployments_list_task,
    prod_tt_sasportal_nodes_nodes_devices_create_builder, prod_tt_sasportal_nodes_nodes_devices_create_task,
    prod_tt_sasportal_nodes_nodes_devices_create_signed_builder, prod_tt_sasportal_nodes_nodes_devices_create_signed_task,
    prod_tt_sasportal_nodes_nodes_devices_list_builder, prod_tt_sasportal_nodes_nodes_devices_list_task,
    prod_tt_sasportal_nodes_nodes_nodes_create_builder, prod_tt_sasportal_nodes_nodes_nodes_create_task,
    prod_tt_sasportal_nodes_nodes_nodes_list_builder, prod_tt_sasportal_nodes_nodes_nodes_list_task,
    prod_tt_sasportal_policies_get_builder, prod_tt_sasportal_policies_get_task,
    prod_tt_sasportal_policies_set_builder, prod_tt_sasportal_policies_set_task,
    prod_tt_sasportal_policies_test_builder, prod_tt_sasportal_policies_test_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::prod_tt_sasportal::SasPortalCustomer;
use crate::providers::gcp::clients::prod_tt_sasportal::SasPortalDeployment;
use crate::providers::gcp::clients::prod_tt_sasportal::SasPortalDevice;
use crate::providers::gcp::clients::prod_tt_sasportal::SasPortalEmpty;
use crate::providers::gcp::clients::prod_tt_sasportal::SasPortalGenerateSecretResponse;
use crate::providers::gcp::clients::prod_tt_sasportal::SasPortalListCustomersResponse;
use crate::providers::gcp::clients::prod_tt_sasportal::SasPortalListDeploymentsResponse;
use crate::providers::gcp::clients::prod_tt_sasportal::SasPortalListDevicesResponse;
use crate::providers::gcp::clients::prod_tt_sasportal::SasPortalListGcpProjectDeploymentsResponse;
use crate::providers::gcp::clients::prod_tt_sasportal::SasPortalListLegacyOrganizationsResponse;
use crate::providers::gcp::clients::prod_tt_sasportal::SasPortalListNodesResponse;
use crate::providers::gcp::clients::prod_tt_sasportal::SasPortalNode;
use crate::providers::gcp::clients::prod_tt_sasportal::SasPortalOperation;
use crate::providers::gcp::clients::prod_tt_sasportal::SasPortalPolicy;
use crate::providers::gcp::clients::prod_tt_sasportal::SasPortalProvisionDeploymentResponse;
use crate::providers::gcp::clients::prod_tt_sasportal::SasPortalTestPermissionsResponse;
use crate::providers::gcp::clients::prod_tt_sasportal::SasPortalValidateInstallerResponse;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersDeploymentsCreateArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersDeploymentsDeleteArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersDeploymentsDevicesCreateArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersDeploymentsDevicesCreateSignedArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersDeploymentsDevicesListArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersDeploymentsGetArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersDeploymentsListArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersDeploymentsMoveArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersDeploymentsPatchArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersDevicesCreateArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersDevicesCreateSignedArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersDevicesDeleteArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersDevicesGetArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersDevicesListArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersDevicesMoveArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersDevicesPatchArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersDevicesSignDeviceArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersDevicesUpdateSignedArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersGetArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersListArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersListGcpProjectDeploymentsArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersListLegacyOrganizationsArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersMigrateOrganizationArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersNodesCreateArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersNodesDeleteArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersNodesDeploymentsCreateArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersNodesDeploymentsListArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersNodesDevicesCreateArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersNodesDevicesCreateSignedArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersNodesDevicesListArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersNodesGetArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersNodesListArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersNodesMoveArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersNodesNodesCreateArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersNodesNodesListArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersNodesPatchArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersPatchArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersProvisionDeploymentArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalCustomersSetupSasAnalyticsArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalDeploymentsDevicesDeleteArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalDeploymentsDevicesGetArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalDeploymentsDevicesMoveArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalDeploymentsDevicesPatchArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalDeploymentsDevicesSignDeviceArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalDeploymentsDevicesUpdateSignedArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalDeploymentsGetArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalInstallerGenerateSecretArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalInstallerValidateArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesDeploymentsDeleteArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesDeploymentsDevicesCreateArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesDeploymentsDevicesCreateSignedArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesDeploymentsDevicesListArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesDeploymentsGetArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesDeploymentsListArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesDeploymentsMoveArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesDeploymentsPatchArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesDevicesCreateArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesDevicesCreateSignedArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesDevicesDeleteArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesDevicesGetArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesDevicesListArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesDevicesMoveArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesDevicesPatchArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesDevicesSignDeviceArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesDevicesUpdateSignedArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesGetArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesNodesCreateArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesNodesDeleteArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesNodesDeploymentsCreateArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesNodesDeploymentsListArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesNodesDevicesCreateArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesNodesDevicesCreateSignedArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesNodesDevicesListArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesNodesGetArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesNodesListArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesNodesMoveArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesNodesNodesCreateArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesNodesNodesListArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalNodesNodesPatchArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalPoliciesGetArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalPoliciesSetArgs;
use crate::providers::gcp::clients::prod_tt_sasportal::ProdTtSasportalPoliciesTestArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ProdTtSasportalProvider with automatic state tracking.
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
/// let provider = ProdTtSasportalProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ProdTtSasportalProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ProdTtSasportalProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ProdTtSasportalProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Prod tt sasportal customers get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalCustomer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_customers_get(
        &self,
        args: &ProdTtSasportalCustomersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalCustomer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalListCustomersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_customers_list(
        &self,
        args: &ProdTtSasportalCustomersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListCustomersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers list gcp project deployments.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalListGcpProjectDeploymentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_customers_list_gcp_project_deployments(
        &self,
        args: &ProdTtSasportalCustomersListGcpProjectDeploymentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListGcpProjectDeploymentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_list_gcp_project_deployments_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_list_gcp_project_deployments_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers list legacy organizations.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalListLegacyOrganizationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_customers_list_legacy_organizations(
        &self,
        args: &ProdTtSasportalCustomersListLegacyOrganizationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListLegacyOrganizationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_list_legacy_organizations_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_list_legacy_organizations_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers migrate organization.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_customers_migrate_organization(
        &self,
        args: &ProdTtSasportalCustomersMigrateOrganizationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_migrate_organization_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_migrate_organization_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalCustomer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_customers_patch(
        &self,
        args: &ProdTtSasportalCustomersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalCustomer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers provision deployment.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalProvisionDeploymentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_customers_provision_deployment(
        &self,
        args: &ProdTtSasportalCustomersProvisionDeploymentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalProvisionDeploymentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_provision_deployment_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_provision_deployment_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers setup sas analytics.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_customers_setup_sas_analytics(
        &self,
        args: &ProdTtSasportalCustomersSetupSasAnalyticsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_setup_sas_analytics_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_setup_sas_analytics_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers deployments create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDeployment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_customers_deployments_create(
        &self,
        args: &ProdTtSasportalCustomersDeploymentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_deployments_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_deployments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers deployments delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_customers_deployments_delete(
        &self,
        args: &ProdTtSasportalCustomersDeploymentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_deployments_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_deployments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers deployments get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDeployment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_customers_deployments_get(
        &self,
        args: &ProdTtSasportalCustomersDeploymentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_deployments_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_deployments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers deployments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalListDeploymentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_customers_deployments_list(
        &self,
        args: &ProdTtSasportalCustomersDeploymentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListDeploymentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_deployments_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_deployments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers deployments move.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_customers_deployments_move(
        &self,
        args: &ProdTtSasportalCustomersDeploymentsMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_deployments_move_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_deployments_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers deployments patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDeployment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_customers_deployments_patch(
        &self,
        args: &ProdTtSasportalCustomersDeploymentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_deployments_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_deployments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers deployments devices create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDevice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_customers_deployments_devices_create(
        &self,
        args: &ProdTtSasportalCustomersDeploymentsDevicesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_deployments_devices_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_deployments_devices_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers deployments devices create signed.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDevice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_customers_deployments_devices_create_signed(
        &self,
        args: &ProdTtSasportalCustomersDeploymentsDevicesCreateSignedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_deployments_devices_create_signed_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_deployments_devices_create_signed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers deployments devices list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalListDevicesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_customers_deployments_devices_list(
        &self,
        args: &ProdTtSasportalCustomersDeploymentsDevicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListDevicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_deployments_devices_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_deployments_devices_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers devices create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDevice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_customers_devices_create(
        &self,
        args: &ProdTtSasportalCustomersDevicesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_devices_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_devices_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers devices create signed.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDevice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_customers_devices_create_signed(
        &self,
        args: &ProdTtSasportalCustomersDevicesCreateSignedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_devices_create_signed_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_devices_create_signed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers devices delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_customers_devices_delete(
        &self,
        args: &ProdTtSasportalCustomersDevicesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_devices_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_devices_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers devices get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDevice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_customers_devices_get(
        &self,
        args: &ProdTtSasportalCustomersDevicesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_devices_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_devices_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers devices list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalListDevicesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_customers_devices_list(
        &self,
        args: &ProdTtSasportalCustomersDevicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListDevicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_devices_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_devices_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers devices move.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_customers_devices_move(
        &self,
        args: &ProdTtSasportalCustomersDevicesMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_devices_move_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_devices_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers devices patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDevice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_customers_devices_patch(
        &self,
        args: &ProdTtSasportalCustomersDevicesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_devices_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_devices_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers devices sign device.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_customers_devices_sign_device(
        &self,
        args: &ProdTtSasportalCustomersDevicesSignDeviceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_devices_sign_device_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_devices_sign_device_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers devices update signed.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDevice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_customers_devices_update_signed(
        &self,
        args: &ProdTtSasportalCustomersDevicesUpdateSignedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_devices_update_signed_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_devices_update_signed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers nodes create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalNode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_customers_nodes_create(
        &self,
        args: &ProdTtSasportalCustomersNodesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalNode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_nodes_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_nodes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers nodes delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_customers_nodes_delete(
        &self,
        args: &ProdTtSasportalCustomersNodesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_nodes_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_nodes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers nodes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalNode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_customers_nodes_get(
        &self,
        args: &ProdTtSasportalCustomersNodesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalNode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_nodes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_nodes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers nodes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalListNodesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_customers_nodes_list(
        &self,
        args: &ProdTtSasportalCustomersNodesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListNodesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_nodes_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_nodes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers nodes move.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_customers_nodes_move(
        &self,
        args: &ProdTtSasportalCustomersNodesMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_nodes_move_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_nodes_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers nodes patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalNode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_customers_nodes_patch(
        &self,
        args: &ProdTtSasportalCustomersNodesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalNode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_nodes_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_nodes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers nodes deployments create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDeployment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_customers_nodes_deployments_create(
        &self,
        args: &ProdTtSasportalCustomersNodesDeploymentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_nodes_deployments_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_nodes_deployments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers nodes deployments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalListDeploymentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_customers_nodes_deployments_list(
        &self,
        args: &ProdTtSasportalCustomersNodesDeploymentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListDeploymentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_nodes_deployments_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_nodes_deployments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers nodes devices create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDevice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_customers_nodes_devices_create(
        &self,
        args: &ProdTtSasportalCustomersNodesDevicesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_nodes_devices_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_nodes_devices_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers nodes devices create signed.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDevice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_customers_nodes_devices_create_signed(
        &self,
        args: &ProdTtSasportalCustomersNodesDevicesCreateSignedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_nodes_devices_create_signed_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_nodes_devices_create_signed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers nodes devices list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalListDevicesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_customers_nodes_devices_list(
        &self,
        args: &ProdTtSasportalCustomersNodesDevicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListDevicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_nodes_devices_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_nodes_devices_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers nodes nodes create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalNode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_customers_nodes_nodes_create(
        &self,
        args: &ProdTtSasportalCustomersNodesNodesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalNode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_nodes_nodes_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_nodes_nodes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal customers nodes nodes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalListNodesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_customers_nodes_nodes_list(
        &self,
        args: &ProdTtSasportalCustomersNodesNodesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListNodesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_customers_nodes_nodes_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_customers_nodes_nodes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal deployments get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDeployment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_deployments_get(
        &self,
        args: &ProdTtSasportalDeploymentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_deployments_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_deployments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal deployments devices delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_deployments_devices_delete(
        &self,
        args: &ProdTtSasportalDeploymentsDevicesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_deployments_devices_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_deployments_devices_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal deployments devices get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDevice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_deployments_devices_get(
        &self,
        args: &ProdTtSasportalDeploymentsDevicesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_deployments_devices_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_deployments_devices_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal deployments devices move.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_deployments_devices_move(
        &self,
        args: &ProdTtSasportalDeploymentsDevicesMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_deployments_devices_move_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_deployments_devices_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal deployments devices patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDevice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_deployments_devices_patch(
        &self,
        args: &ProdTtSasportalDeploymentsDevicesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_deployments_devices_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_deployments_devices_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal deployments devices sign device.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_deployments_devices_sign_device(
        &self,
        args: &ProdTtSasportalDeploymentsDevicesSignDeviceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_deployments_devices_sign_device_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_deployments_devices_sign_device_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal deployments devices update signed.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDevice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_deployments_devices_update_signed(
        &self,
        args: &ProdTtSasportalDeploymentsDevicesUpdateSignedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_deployments_devices_update_signed_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_deployments_devices_update_signed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal installer generate secret.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalGenerateSecretResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_installer_generate_secret(
        &self,
        args: &ProdTtSasportalInstallerGenerateSecretArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalGenerateSecretResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_installer_generate_secret_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_installer_generate_secret_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal installer validate.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalValidateInstallerResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_installer_validate(
        &self,
        args: &ProdTtSasportalInstallerValidateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalValidateInstallerResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_installer_validate_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_installer_validate_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalNode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_nodes_get(
        &self,
        args: &ProdTtSasportalNodesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalNode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes deployments delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_nodes_deployments_delete(
        &self,
        args: &ProdTtSasportalNodesDeploymentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_deployments_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_deployments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes deployments get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDeployment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_nodes_deployments_get(
        &self,
        args: &ProdTtSasportalNodesDeploymentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_deployments_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_deployments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes deployments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalListDeploymentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_nodes_deployments_list(
        &self,
        args: &ProdTtSasportalNodesDeploymentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListDeploymentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_deployments_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_deployments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes deployments move.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_nodes_deployments_move(
        &self,
        args: &ProdTtSasportalNodesDeploymentsMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_deployments_move_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_deployments_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes deployments patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDeployment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_nodes_deployments_patch(
        &self,
        args: &ProdTtSasportalNodesDeploymentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_deployments_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_deployments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes deployments devices create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDevice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_nodes_deployments_devices_create(
        &self,
        args: &ProdTtSasportalNodesDeploymentsDevicesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_deployments_devices_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_deployments_devices_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes deployments devices create signed.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDevice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_nodes_deployments_devices_create_signed(
        &self,
        args: &ProdTtSasportalNodesDeploymentsDevicesCreateSignedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_deployments_devices_create_signed_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_deployments_devices_create_signed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes deployments devices list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalListDevicesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_nodes_deployments_devices_list(
        &self,
        args: &ProdTtSasportalNodesDeploymentsDevicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListDevicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_deployments_devices_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_deployments_devices_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes devices create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDevice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_nodes_devices_create(
        &self,
        args: &ProdTtSasportalNodesDevicesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_devices_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_devices_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes devices create signed.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDevice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_nodes_devices_create_signed(
        &self,
        args: &ProdTtSasportalNodesDevicesCreateSignedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_devices_create_signed_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_devices_create_signed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes devices delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_nodes_devices_delete(
        &self,
        args: &ProdTtSasportalNodesDevicesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_devices_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_devices_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes devices get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDevice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_nodes_devices_get(
        &self,
        args: &ProdTtSasportalNodesDevicesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_devices_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_devices_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes devices list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalListDevicesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_nodes_devices_list(
        &self,
        args: &ProdTtSasportalNodesDevicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListDevicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_devices_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_devices_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes devices move.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_nodes_devices_move(
        &self,
        args: &ProdTtSasportalNodesDevicesMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_devices_move_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_devices_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes devices patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDevice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_nodes_devices_patch(
        &self,
        args: &ProdTtSasportalNodesDevicesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_devices_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_devices_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes devices sign device.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_nodes_devices_sign_device(
        &self,
        args: &ProdTtSasportalNodesDevicesSignDeviceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_devices_sign_device_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_devices_sign_device_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes devices update signed.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDevice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_nodes_devices_update_signed(
        &self,
        args: &ProdTtSasportalNodesDevicesUpdateSignedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_devices_update_signed_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_devices_update_signed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes nodes create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalNode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_nodes_nodes_create(
        &self,
        args: &ProdTtSasportalNodesNodesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalNode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_nodes_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_nodes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes nodes delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_nodes_nodes_delete(
        &self,
        args: &ProdTtSasportalNodesNodesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_nodes_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_nodes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes nodes get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalNode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_nodes_nodes_get(
        &self,
        args: &ProdTtSasportalNodesNodesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalNode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_nodes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_nodes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes nodes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalListNodesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_nodes_nodes_list(
        &self,
        args: &ProdTtSasportalNodesNodesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListNodesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_nodes_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_nodes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes nodes move.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_nodes_nodes_move(
        &self,
        args: &ProdTtSasportalNodesNodesMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_nodes_move_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_nodes_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes nodes patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalNode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_nodes_nodes_patch(
        &self,
        args: &ProdTtSasportalNodesNodesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalNode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_nodes_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_nodes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes nodes deployments create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDeployment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_nodes_nodes_deployments_create(
        &self,
        args: &ProdTtSasportalNodesNodesDeploymentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_nodes_deployments_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_nodes_deployments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes nodes deployments list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalListDeploymentsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_nodes_nodes_deployments_list(
        &self,
        args: &ProdTtSasportalNodesNodesDeploymentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListDeploymentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_nodes_deployments_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_nodes_deployments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes nodes devices create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDevice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_nodes_nodes_devices_create(
        &self,
        args: &ProdTtSasportalNodesNodesDevicesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_nodes_devices_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_nodes_devices_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes nodes devices create signed.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalDevice result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_nodes_nodes_devices_create_signed(
        &self,
        args: &ProdTtSasportalNodesNodesDevicesCreateSignedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_nodes_devices_create_signed_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_nodes_devices_create_signed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes nodes devices list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalListDevicesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_nodes_nodes_devices_list(
        &self,
        args: &ProdTtSasportalNodesNodesDevicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListDevicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_nodes_devices_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_nodes_devices_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes nodes nodes create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalNode result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_nodes_nodes_nodes_create(
        &self,
        args: &ProdTtSasportalNodesNodesNodesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalNode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_nodes_nodes_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_nodes_nodes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal nodes nodes nodes list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalListNodesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_nodes_nodes_nodes_list(
        &self,
        args: &ProdTtSasportalNodesNodesNodesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListNodesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_nodes_nodes_nodes_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_nodes_nodes_nodes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal policies get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_policies_get(
        &self,
        args: &ProdTtSasportalPoliciesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_policies_get_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_policies_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal policies set.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn prod_tt_sasportal_policies_set(
        &self,
        args: &ProdTtSasportalPoliciesSetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_policies_set_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_policies_set_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Prod tt sasportal policies test.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SasPortalTestPermissionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn prod_tt_sasportal_policies_test(
        &self,
        args: &ProdTtSasportalPoliciesTestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalTestPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = prod_tt_sasportal_policies_test_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = prod_tt_sasportal_policies_test_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
