//! SasportalProvider - State-aware sasportal API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       sasportal API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::sasportal::{
    sasportal_customers_get_builder, sasportal_customers_get_task,
    sasportal_customers_list_builder, sasportal_customers_list_task,
    sasportal_customers_list_gcp_project_deployments_builder, sasportal_customers_list_gcp_project_deployments_task,
    sasportal_customers_list_legacy_organizations_builder, sasportal_customers_list_legacy_organizations_task,
    sasportal_customers_migrate_organization_builder, sasportal_customers_migrate_organization_task,
    sasportal_customers_patch_builder, sasportal_customers_patch_task,
    sasportal_customers_provision_deployment_builder, sasportal_customers_provision_deployment_task,
    sasportal_customers_setup_sas_analytics_builder, sasportal_customers_setup_sas_analytics_task,
    sasportal_customers_deployments_create_builder, sasportal_customers_deployments_create_task,
    sasportal_customers_deployments_delete_builder, sasportal_customers_deployments_delete_task,
    sasportal_customers_deployments_get_builder, sasportal_customers_deployments_get_task,
    sasportal_customers_deployments_list_builder, sasportal_customers_deployments_list_task,
    sasportal_customers_deployments_move_builder, sasportal_customers_deployments_move_task,
    sasportal_customers_deployments_patch_builder, sasportal_customers_deployments_patch_task,
    sasportal_customers_deployments_devices_create_builder, sasportal_customers_deployments_devices_create_task,
    sasportal_customers_deployments_devices_create_signed_builder, sasportal_customers_deployments_devices_create_signed_task,
    sasportal_customers_deployments_devices_list_builder, sasportal_customers_deployments_devices_list_task,
    sasportal_customers_devices_create_builder, sasportal_customers_devices_create_task,
    sasportal_customers_devices_create_signed_builder, sasportal_customers_devices_create_signed_task,
    sasportal_customers_devices_delete_builder, sasportal_customers_devices_delete_task,
    sasportal_customers_devices_get_builder, sasportal_customers_devices_get_task,
    sasportal_customers_devices_list_builder, sasportal_customers_devices_list_task,
    sasportal_customers_devices_move_builder, sasportal_customers_devices_move_task,
    sasportal_customers_devices_patch_builder, sasportal_customers_devices_patch_task,
    sasportal_customers_devices_sign_device_builder, sasportal_customers_devices_sign_device_task,
    sasportal_customers_devices_update_signed_builder, sasportal_customers_devices_update_signed_task,
    sasportal_customers_nodes_create_builder, sasportal_customers_nodes_create_task,
    sasportal_customers_nodes_delete_builder, sasportal_customers_nodes_delete_task,
    sasportal_customers_nodes_get_builder, sasportal_customers_nodes_get_task,
    sasportal_customers_nodes_list_builder, sasportal_customers_nodes_list_task,
    sasportal_customers_nodes_move_builder, sasportal_customers_nodes_move_task,
    sasportal_customers_nodes_patch_builder, sasportal_customers_nodes_patch_task,
    sasportal_customers_nodes_deployments_create_builder, sasportal_customers_nodes_deployments_create_task,
    sasportal_customers_nodes_deployments_list_builder, sasportal_customers_nodes_deployments_list_task,
    sasportal_customers_nodes_devices_create_builder, sasportal_customers_nodes_devices_create_task,
    sasportal_customers_nodes_devices_create_signed_builder, sasportal_customers_nodes_devices_create_signed_task,
    sasportal_customers_nodes_devices_list_builder, sasportal_customers_nodes_devices_list_task,
    sasportal_customers_nodes_nodes_create_builder, sasportal_customers_nodes_nodes_create_task,
    sasportal_customers_nodes_nodes_list_builder, sasportal_customers_nodes_nodes_list_task,
    sasportal_deployments_get_builder, sasportal_deployments_get_task,
    sasportal_deployments_devices_delete_builder, sasportal_deployments_devices_delete_task,
    sasportal_deployments_devices_get_builder, sasportal_deployments_devices_get_task,
    sasportal_deployments_devices_move_builder, sasportal_deployments_devices_move_task,
    sasportal_deployments_devices_patch_builder, sasportal_deployments_devices_patch_task,
    sasportal_deployments_devices_sign_device_builder, sasportal_deployments_devices_sign_device_task,
    sasportal_deployments_devices_update_signed_builder, sasportal_deployments_devices_update_signed_task,
    sasportal_installer_generate_secret_builder, sasportal_installer_generate_secret_task,
    sasportal_installer_validate_builder, sasportal_installer_validate_task,
    sasportal_nodes_get_builder, sasportal_nodes_get_task,
    sasportal_nodes_deployments_delete_builder, sasportal_nodes_deployments_delete_task,
    sasportal_nodes_deployments_get_builder, sasportal_nodes_deployments_get_task,
    sasportal_nodes_deployments_list_builder, sasportal_nodes_deployments_list_task,
    sasportal_nodes_deployments_move_builder, sasportal_nodes_deployments_move_task,
    sasportal_nodes_deployments_patch_builder, sasportal_nodes_deployments_patch_task,
    sasportal_nodes_deployments_devices_create_builder, sasportal_nodes_deployments_devices_create_task,
    sasportal_nodes_deployments_devices_create_signed_builder, sasportal_nodes_deployments_devices_create_signed_task,
    sasportal_nodes_deployments_devices_list_builder, sasportal_nodes_deployments_devices_list_task,
    sasportal_nodes_devices_create_builder, sasportal_nodes_devices_create_task,
    sasportal_nodes_devices_create_signed_builder, sasportal_nodes_devices_create_signed_task,
    sasportal_nodes_devices_delete_builder, sasportal_nodes_devices_delete_task,
    sasportal_nodes_devices_get_builder, sasportal_nodes_devices_get_task,
    sasportal_nodes_devices_list_builder, sasportal_nodes_devices_list_task,
    sasportal_nodes_devices_move_builder, sasportal_nodes_devices_move_task,
    sasportal_nodes_devices_patch_builder, sasportal_nodes_devices_patch_task,
    sasportal_nodes_devices_sign_device_builder, sasportal_nodes_devices_sign_device_task,
    sasportal_nodes_devices_update_signed_builder, sasportal_nodes_devices_update_signed_task,
    sasportal_nodes_nodes_create_builder, sasportal_nodes_nodes_create_task,
    sasportal_nodes_nodes_delete_builder, sasportal_nodes_nodes_delete_task,
    sasportal_nodes_nodes_get_builder, sasportal_nodes_nodes_get_task,
    sasportal_nodes_nodes_list_builder, sasportal_nodes_nodes_list_task,
    sasportal_nodes_nodes_move_builder, sasportal_nodes_nodes_move_task,
    sasportal_nodes_nodes_patch_builder, sasportal_nodes_nodes_patch_task,
    sasportal_nodes_nodes_deployments_create_builder, sasportal_nodes_nodes_deployments_create_task,
    sasportal_nodes_nodes_deployments_list_builder, sasportal_nodes_nodes_deployments_list_task,
    sasportal_nodes_nodes_devices_create_builder, sasportal_nodes_nodes_devices_create_task,
    sasportal_nodes_nodes_devices_create_signed_builder, sasportal_nodes_nodes_devices_create_signed_task,
    sasportal_nodes_nodes_devices_list_builder, sasportal_nodes_nodes_devices_list_task,
    sasportal_nodes_nodes_nodes_create_builder, sasportal_nodes_nodes_nodes_create_task,
    sasportal_nodes_nodes_nodes_list_builder, sasportal_nodes_nodes_nodes_list_task,
    sasportal_policies_get_builder, sasportal_policies_get_task,
    sasportal_policies_set_builder, sasportal_policies_set_task,
    sasportal_policies_test_builder, sasportal_policies_test_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::sasportal::SasPortalCustomer;
use crate::providers::gcp::clients::sasportal::SasPortalDeployment;
use crate::providers::gcp::clients::sasportal::SasPortalDevice;
use crate::providers::gcp::clients::sasportal::SasPortalEmpty;
use crate::providers::gcp::clients::sasportal::SasPortalGenerateSecretResponse;
use crate::providers::gcp::clients::sasportal::SasPortalListCustomersResponse;
use crate::providers::gcp::clients::sasportal::SasPortalListDeploymentsResponse;
use crate::providers::gcp::clients::sasportal::SasPortalListDevicesResponse;
use crate::providers::gcp::clients::sasportal::SasPortalListGcpProjectDeploymentsResponse;
use crate::providers::gcp::clients::sasportal::SasPortalListLegacyOrganizationsResponse;
use crate::providers::gcp::clients::sasportal::SasPortalListNodesResponse;
use crate::providers::gcp::clients::sasportal::SasPortalNode;
use crate::providers::gcp::clients::sasportal::SasPortalOperation;
use crate::providers::gcp::clients::sasportal::SasPortalPolicy;
use crate::providers::gcp::clients::sasportal::SasPortalProvisionDeploymentResponse;
use crate::providers::gcp::clients::sasportal::SasPortalTestPermissionsResponse;
use crate::providers::gcp::clients::sasportal::SasPortalValidateInstallerResponse;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDeploymentsCreateArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDeploymentsDeleteArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDeploymentsDevicesCreateArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDeploymentsDevicesCreateSignedArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDeploymentsDevicesListArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDeploymentsGetArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDeploymentsListArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDeploymentsMoveArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDeploymentsPatchArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDevicesCreateArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDevicesCreateSignedArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDevicesDeleteArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDevicesGetArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDevicesListArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDevicesMoveArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDevicesPatchArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDevicesSignDeviceArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDevicesUpdateSignedArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersGetArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersListArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersNodesCreateArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersNodesDeleteArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersNodesDeploymentsCreateArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersNodesDeploymentsListArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersNodesDevicesCreateArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersNodesDevicesCreateSignedArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersNodesDevicesListArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersNodesGetArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersNodesListArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersNodesMoveArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersNodesNodesCreateArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersNodesNodesListArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersNodesPatchArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersPatchArgs;
use crate::providers::gcp::clients::sasportal::SasportalDeploymentsDevicesDeleteArgs;
use crate::providers::gcp::clients::sasportal::SasportalDeploymentsDevicesGetArgs;
use crate::providers::gcp::clients::sasportal::SasportalDeploymentsDevicesMoveArgs;
use crate::providers::gcp::clients::sasportal::SasportalDeploymentsDevicesPatchArgs;
use crate::providers::gcp::clients::sasportal::SasportalDeploymentsDevicesSignDeviceArgs;
use crate::providers::gcp::clients::sasportal::SasportalDeploymentsDevicesUpdateSignedArgs;
use crate::providers::gcp::clients::sasportal::SasportalDeploymentsGetArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDeploymentsDeleteArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDeploymentsDevicesCreateArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDeploymentsDevicesCreateSignedArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDeploymentsDevicesListArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDeploymentsGetArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDeploymentsListArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDeploymentsMoveArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDeploymentsPatchArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDevicesCreateArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDevicesCreateSignedArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDevicesDeleteArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDevicesGetArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDevicesListArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDevicesMoveArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDevicesPatchArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDevicesSignDeviceArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDevicesUpdateSignedArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesGetArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesNodesCreateArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesNodesDeleteArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesNodesDeploymentsCreateArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesNodesDeploymentsListArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesNodesDevicesCreateArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesNodesDevicesCreateSignedArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesNodesDevicesListArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesNodesGetArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesNodesListArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesNodesMoveArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesNodesNodesCreateArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesNodesNodesListArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesNodesPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// SasportalProvider with automatic state tracking.
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
/// let provider = SasportalProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct SasportalProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> SasportalProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new SasportalProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new SasportalProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Sasportal customers get.
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
    pub fn sasportal_customers_get(
        &self,
        args: &SasportalCustomersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalCustomer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers list.
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
    pub fn sasportal_customers_list(
        &self,
        args: &SasportalCustomersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListCustomersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers list gcp project deployments.
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
    pub fn sasportal_customers_list_gcp_project_deployments(
        &self,
        args: &SasportalCustomersListGcpProjectDeploymentsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListGcpProjectDeploymentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_list_gcp_project_deployments_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_list_gcp_project_deployments_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers list legacy organizations.
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
    pub fn sasportal_customers_list_legacy_organizations(
        &self,
        args: &SasportalCustomersListLegacyOrganizationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListLegacyOrganizationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_list_legacy_organizations_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_list_legacy_organizations_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers migrate organization.
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
    pub fn sasportal_customers_migrate_organization(
        &self,
        args: &SasportalCustomersMigrateOrganizationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_migrate_organization_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_migrate_organization_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers patch.
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
    pub fn sasportal_customers_patch(
        &self,
        args: &SasportalCustomersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalCustomer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers provision deployment.
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
    pub fn sasportal_customers_provision_deployment(
        &self,
        args: &SasportalCustomersProvisionDeploymentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalProvisionDeploymentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_provision_deployment_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_provision_deployment_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers setup sas analytics.
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
    pub fn sasportal_customers_setup_sas_analytics(
        &self,
        args: &SasportalCustomersSetupSasAnalyticsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_setup_sas_analytics_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_setup_sas_analytics_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers deployments create.
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
    pub fn sasportal_customers_deployments_create(
        &self,
        args: &SasportalCustomersDeploymentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_deployments_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_deployments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers deployments delete.
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
    pub fn sasportal_customers_deployments_delete(
        &self,
        args: &SasportalCustomersDeploymentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_deployments_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_deployments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers deployments get.
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
    pub fn sasportal_customers_deployments_get(
        &self,
        args: &SasportalCustomersDeploymentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_deployments_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_deployments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers deployments list.
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
    pub fn sasportal_customers_deployments_list(
        &self,
        args: &SasportalCustomersDeploymentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListDeploymentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_deployments_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_deployments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers deployments move.
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
    pub fn sasportal_customers_deployments_move(
        &self,
        args: &SasportalCustomersDeploymentsMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_deployments_move_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_deployments_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers deployments patch.
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
    pub fn sasportal_customers_deployments_patch(
        &self,
        args: &SasportalCustomersDeploymentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_deployments_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_deployments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers deployments devices create.
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
    pub fn sasportal_customers_deployments_devices_create(
        &self,
        args: &SasportalCustomersDeploymentsDevicesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_deployments_devices_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_deployments_devices_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers deployments devices create signed.
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
    pub fn sasportal_customers_deployments_devices_create_signed(
        &self,
        args: &SasportalCustomersDeploymentsDevicesCreateSignedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_deployments_devices_create_signed_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_deployments_devices_create_signed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers deployments devices list.
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
    pub fn sasportal_customers_deployments_devices_list(
        &self,
        args: &SasportalCustomersDeploymentsDevicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListDevicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_deployments_devices_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_deployments_devices_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers devices create.
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
    pub fn sasportal_customers_devices_create(
        &self,
        args: &SasportalCustomersDevicesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_devices_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_devices_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers devices create signed.
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
    pub fn sasportal_customers_devices_create_signed(
        &self,
        args: &SasportalCustomersDevicesCreateSignedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_devices_create_signed_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_devices_create_signed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers devices delete.
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
    pub fn sasportal_customers_devices_delete(
        &self,
        args: &SasportalCustomersDevicesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_devices_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_devices_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers devices get.
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
    pub fn sasportal_customers_devices_get(
        &self,
        args: &SasportalCustomersDevicesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_devices_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_devices_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers devices list.
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
    pub fn sasportal_customers_devices_list(
        &self,
        args: &SasportalCustomersDevicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListDevicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_devices_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_devices_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers devices move.
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
    pub fn sasportal_customers_devices_move(
        &self,
        args: &SasportalCustomersDevicesMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_devices_move_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_devices_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers devices patch.
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
    pub fn sasportal_customers_devices_patch(
        &self,
        args: &SasportalCustomersDevicesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_devices_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_devices_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers devices sign device.
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
    pub fn sasportal_customers_devices_sign_device(
        &self,
        args: &SasportalCustomersDevicesSignDeviceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_devices_sign_device_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_devices_sign_device_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers devices update signed.
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
    pub fn sasportal_customers_devices_update_signed(
        &self,
        args: &SasportalCustomersDevicesUpdateSignedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_devices_update_signed_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_devices_update_signed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers nodes create.
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
    pub fn sasportal_customers_nodes_create(
        &self,
        args: &SasportalCustomersNodesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalNode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_nodes_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_nodes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers nodes delete.
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
    pub fn sasportal_customers_nodes_delete(
        &self,
        args: &SasportalCustomersNodesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_nodes_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_nodes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers nodes get.
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
    pub fn sasportal_customers_nodes_get(
        &self,
        args: &SasportalCustomersNodesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalNode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_nodes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_nodes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers nodes list.
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
    pub fn sasportal_customers_nodes_list(
        &self,
        args: &SasportalCustomersNodesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListNodesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_nodes_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_nodes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers nodes move.
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
    pub fn sasportal_customers_nodes_move(
        &self,
        args: &SasportalCustomersNodesMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_nodes_move_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_nodes_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers nodes patch.
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
    pub fn sasportal_customers_nodes_patch(
        &self,
        args: &SasportalCustomersNodesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalNode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_nodes_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_nodes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers nodes deployments create.
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
    pub fn sasportal_customers_nodes_deployments_create(
        &self,
        args: &SasportalCustomersNodesDeploymentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_nodes_deployments_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_nodes_deployments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers nodes deployments list.
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
    pub fn sasportal_customers_nodes_deployments_list(
        &self,
        args: &SasportalCustomersNodesDeploymentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListDeploymentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_nodes_deployments_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_nodes_deployments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers nodes devices create.
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
    pub fn sasportal_customers_nodes_devices_create(
        &self,
        args: &SasportalCustomersNodesDevicesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_nodes_devices_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_nodes_devices_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers nodes devices create signed.
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
    pub fn sasportal_customers_nodes_devices_create_signed(
        &self,
        args: &SasportalCustomersNodesDevicesCreateSignedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_nodes_devices_create_signed_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_nodes_devices_create_signed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers nodes devices list.
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
    pub fn sasportal_customers_nodes_devices_list(
        &self,
        args: &SasportalCustomersNodesDevicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListDevicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_nodes_devices_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_nodes_devices_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers nodes nodes create.
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
    pub fn sasportal_customers_nodes_nodes_create(
        &self,
        args: &SasportalCustomersNodesNodesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalNode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_nodes_nodes_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_nodes_nodes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal customers nodes nodes list.
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
    pub fn sasportal_customers_nodes_nodes_list(
        &self,
        args: &SasportalCustomersNodesNodesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListNodesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_customers_nodes_nodes_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_customers_nodes_nodes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal deployments get.
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
    pub fn sasportal_deployments_get(
        &self,
        args: &SasportalDeploymentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_deployments_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_deployments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal deployments devices delete.
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
    pub fn sasportal_deployments_devices_delete(
        &self,
        args: &SasportalDeploymentsDevicesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_deployments_devices_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_deployments_devices_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal deployments devices get.
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
    pub fn sasportal_deployments_devices_get(
        &self,
        args: &SasportalDeploymentsDevicesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_deployments_devices_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_deployments_devices_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal deployments devices move.
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
    pub fn sasportal_deployments_devices_move(
        &self,
        args: &SasportalDeploymentsDevicesMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_deployments_devices_move_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_deployments_devices_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal deployments devices patch.
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
    pub fn sasportal_deployments_devices_patch(
        &self,
        args: &SasportalDeploymentsDevicesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_deployments_devices_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_deployments_devices_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal deployments devices sign device.
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
    pub fn sasportal_deployments_devices_sign_device(
        &self,
        args: &SasportalDeploymentsDevicesSignDeviceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_deployments_devices_sign_device_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_deployments_devices_sign_device_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal deployments devices update signed.
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
    pub fn sasportal_deployments_devices_update_signed(
        &self,
        args: &SasportalDeploymentsDevicesUpdateSignedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_deployments_devices_update_signed_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_deployments_devices_update_signed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal installer generate secret.
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
    pub fn sasportal_installer_generate_secret(
        &self,
        args: &SasportalInstallerGenerateSecretArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalGenerateSecretResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_installer_generate_secret_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_installer_generate_secret_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal installer validate.
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
    pub fn sasportal_installer_validate(
        &self,
        args: &SasportalInstallerValidateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalValidateInstallerResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_installer_validate_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_installer_validate_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes get.
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
    pub fn sasportal_nodes_get(
        &self,
        args: &SasportalNodesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalNode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes deployments delete.
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
    pub fn sasportal_nodes_deployments_delete(
        &self,
        args: &SasportalNodesDeploymentsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_deployments_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_deployments_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes deployments get.
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
    pub fn sasportal_nodes_deployments_get(
        &self,
        args: &SasportalNodesDeploymentsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_deployments_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_deployments_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes deployments list.
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
    pub fn sasportal_nodes_deployments_list(
        &self,
        args: &SasportalNodesDeploymentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListDeploymentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_deployments_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_deployments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes deployments move.
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
    pub fn sasportal_nodes_deployments_move(
        &self,
        args: &SasportalNodesDeploymentsMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_deployments_move_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_deployments_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes deployments patch.
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
    pub fn sasportal_nodes_deployments_patch(
        &self,
        args: &SasportalNodesDeploymentsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_deployments_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_deployments_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes deployments devices create.
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
    pub fn sasportal_nodes_deployments_devices_create(
        &self,
        args: &SasportalNodesDeploymentsDevicesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_deployments_devices_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_deployments_devices_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes deployments devices create signed.
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
    pub fn sasportal_nodes_deployments_devices_create_signed(
        &self,
        args: &SasportalNodesDeploymentsDevicesCreateSignedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_deployments_devices_create_signed_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_deployments_devices_create_signed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes deployments devices list.
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
    pub fn sasportal_nodes_deployments_devices_list(
        &self,
        args: &SasportalNodesDeploymentsDevicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListDevicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_deployments_devices_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_deployments_devices_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes devices create.
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
    pub fn sasportal_nodes_devices_create(
        &self,
        args: &SasportalNodesDevicesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_devices_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_devices_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes devices create signed.
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
    pub fn sasportal_nodes_devices_create_signed(
        &self,
        args: &SasportalNodesDevicesCreateSignedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_devices_create_signed_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_devices_create_signed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes devices delete.
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
    pub fn sasportal_nodes_devices_delete(
        &self,
        args: &SasportalNodesDevicesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_devices_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_devices_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes devices get.
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
    pub fn sasportal_nodes_devices_get(
        &self,
        args: &SasportalNodesDevicesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_devices_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_devices_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes devices list.
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
    pub fn sasportal_nodes_devices_list(
        &self,
        args: &SasportalNodesDevicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListDevicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_devices_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_devices_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes devices move.
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
    pub fn sasportal_nodes_devices_move(
        &self,
        args: &SasportalNodesDevicesMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_devices_move_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_devices_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes devices patch.
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
    pub fn sasportal_nodes_devices_patch(
        &self,
        args: &SasportalNodesDevicesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_devices_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_devices_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes devices sign device.
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
    pub fn sasportal_nodes_devices_sign_device(
        &self,
        args: &SasportalNodesDevicesSignDeviceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_devices_sign_device_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_devices_sign_device_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes devices update signed.
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
    pub fn sasportal_nodes_devices_update_signed(
        &self,
        args: &SasportalNodesDevicesUpdateSignedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_devices_update_signed_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_devices_update_signed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes nodes create.
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
    pub fn sasportal_nodes_nodes_create(
        &self,
        args: &SasportalNodesNodesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalNode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_nodes_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_nodes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes nodes delete.
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
    pub fn sasportal_nodes_nodes_delete(
        &self,
        args: &SasportalNodesNodesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_nodes_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_nodes_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes nodes get.
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
    pub fn sasportal_nodes_nodes_get(
        &self,
        args: &SasportalNodesNodesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalNode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_nodes_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_nodes_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes nodes list.
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
    pub fn sasportal_nodes_nodes_list(
        &self,
        args: &SasportalNodesNodesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListNodesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_nodes_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_nodes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes nodes move.
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
    pub fn sasportal_nodes_nodes_move(
        &self,
        args: &SasportalNodesNodesMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_nodes_move_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_nodes_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes nodes patch.
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
    pub fn sasportal_nodes_nodes_patch(
        &self,
        args: &SasportalNodesNodesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalNode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_nodes_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_nodes_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes nodes deployments create.
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
    pub fn sasportal_nodes_nodes_deployments_create(
        &self,
        args: &SasportalNodesNodesDeploymentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDeployment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_nodes_deployments_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_nodes_deployments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes nodes deployments list.
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
    pub fn sasportal_nodes_nodes_deployments_list(
        &self,
        args: &SasportalNodesNodesDeploymentsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListDeploymentsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_nodes_deployments_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_nodes_deployments_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes nodes devices create.
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
    pub fn sasportal_nodes_nodes_devices_create(
        &self,
        args: &SasportalNodesNodesDevicesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_nodes_devices_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_nodes_devices_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes nodes devices create signed.
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
    pub fn sasportal_nodes_nodes_devices_create_signed(
        &self,
        args: &SasportalNodesNodesDevicesCreateSignedArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalDevice, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_nodes_devices_create_signed_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_nodes_devices_create_signed_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes nodes devices list.
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
    pub fn sasportal_nodes_nodes_devices_list(
        &self,
        args: &SasportalNodesNodesDevicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListDevicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_nodes_devices_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_nodes_devices_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes nodes nodes create.
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
    pub fn sasportal_nodes_nodes_nodes_create(
        &self,
        args: &SasportalNodesNodesNodesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalNode, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_nodes_nodes_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_nodes_nodes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal nodes nodes nodes list.
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
    pub fn sasportal_nodes_nodes_nodes_list(
        &self,
        args: &SasportalNodesNodesNodesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalListNodesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_nodes_nodes_nodes_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_nodes_nodes_nodes_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal policies get.
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
    pub fn sasportal_policies_get(
        &self,
        args: &SasportalPoliciesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_policies_get_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_policies_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal policies set.
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
    pub fn sasportal_policies_set(
        &self,
        args: &SasportalPoliciesSetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_policies_set_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_policies_set_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Sasportal policies test.
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
    pub fn sasportal_policies_test(
        &self,
        args: &SasportalPoliciesTestArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SasPortalTestPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = sasportal_policies_test_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = sasportal_policies_test_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
