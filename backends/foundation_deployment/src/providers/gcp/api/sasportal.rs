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
    sasportal_customers_migrate_organization_builder, sasportal_customers_migrate_organization_task,
    sasportal_customers_patch_builder, sasportal_customers_patch_task,
    sasportal_customers_provision_deployment_builder, sasportal_customers_provision_deployment_task,
    sasportal_customers_setup_sas_analytics_builder, sasportal_customers_setup_sas_analytics_task,
    sasportal_customers_deployments_create_builder, sasportal_customers_deployments_create_task,
    sasportal_customers_deployments_delete_builder, sasportal_customers_deployments_delete_task,
    sasportal_customers_deployments_move_builder, sasportal_customers_deployments_move_task,
    sasportal_customers_deployments_patch_builder, sasportal_customers_deployments_patch_task,
    sasportal_customers_deployments_devices_create_builder, sasportal_customers_deployments_devices_create_task,
    sasportal_customers_deployments_devices_create_signed_builder, sasportal_customers_deployments_devices_create_signed_task,
    sasportal_customers_devices_create_builder, sasportal_customers_devices_create_task,
    sasportal_customers_devices_create_signed_builder, sasportal_customers_devices_create_signed_task,
    sasportal_customers_devices_delete_builder, sasportal_customers_devices_delete_task,
    sasportal_customers_devices_move_builder, sasportal_customers_devices_move_task,
    sasportal_customers_devices_patch_builder, sasportal_customers_devices_patch_task,
    sasportal_customers_devices_sign_device_builder, sasportal_customers_devices_sign_device_task,
    sasportal_customers_devices_update_signed_builder, sasportal_customers_devices_update_signed_task,
    sasportal_customers_nodes_create_builder, sasportal_customers_nodes_create_task,
    sasportal_customers_nodes_delete_builder, sasportal_customers_nodes_delete_task,
    sasportal_customers_nodes_move_builder, sasportal_customers_nodes_move_task,
    sasportal_customers_nodes_patch_builder, sasportal_customers_nodes_patch_task,
    sasportal_customers_nodes_deployments_create_builder, sasportal_customers_nodes_deployments_create_task,
    sasportal_customers_nodes_devices_create_builder, sasportal_customers_nodes_devices_create_task,
    sasportal_customers_nodes_devices_create_signed_builder, sasportal_customers_nodes_devices_create_signed_task,
    sasportal_customers_nodes_nodes_create_builder, sasportal_customers_nodes_nodes_create_task,
    sasportal_deployments_devices_delete_builder, sasportal_deployments_devices_delete_task,
    sasportal_deployments_devices_move_builder, sasportal_deployments_devices_move_task,
    sasportal_deployments_devices_patch_builder, sasportal_deployments_devices_patch_task,
    sasportal_deployments_devices_sign_device_builder, sasportal_deployments_devices_sign_device_task,
    sasportal_deployments_devices_update_signed_builder, sasportal_deployments_devices_update_signed_task,
    sasportal_installer_generate_secret_builder, sasportal_installer_generate_secret_task,
    sasportal_installer_validate_builder, sasportal_installer_validate_task,
    sasportal_nodes_deployments_delete_builder, sasportal_nodes_deployments_delete_task,
    sasportal_nodes_deployments_move_builder, sasportal_nodes_deployments_move_task,
    sasportal_nodes_deployments_patch_builder, sasportal_nodes_deployments_patch_task,
    sasportal_nodes_deployments_devices_create_builder, sasportal_nodes_deployments_devices_create_task,
    sasportal_nodes_deployments_devices_create_signed_builder, sasportal_nodes_deployments_devices_create_signed_task,
    sasportal_nodes_devices_create_builder, sasportal_nodes_devices_create_task,
    sasportal_nodes_devices_create_signed_builder, sasportal_nodes_devices_create_signed_task,
    sasportal_nodes_devices_delete_builder, sasportal_nodes_devices_delete_task,
    sasportal_nodes_devices_move_builder, sasportal_nodes_devices_move_task,
    sasportal_nodes_devices_patch_builder, sasportal_nodes_devices_patch_task,
    sasportal_nodes_devices_sign_device_builder, sasportal_nodes_devices_sign_device_task,
    sasportal_nodes_devices_update_signed_builder, sasportal_nodes_devices_update_signed_task,
    sasportal_nodes_nodes_create_builder, sasportal_nodes_nodes_create_task,
    sasportal_nodes_nodes_delete_builder, sasportal_nodes_nodes_delete_task,
    sasportal_nodes_nodes_move_builder, sasportal_nodes_nodes_move_task,
    sasportal_nodes_nodes_patch_builder, sasportal_nodes_nodes_patch_task,
    sasportal_nodes_nodes_deployments_create_builder, sasportal_nodes_nodes_deployments_create_task,
    sasportal_nodes_nodes_devices_create_builder, sasportal_nodes_nodes_devices_create_task,
    sasportal_nodes_nodes_devices_create_signed_builder, sasportal_nodes_nodes_devices_create_signed_task,
    sasportal_nodes_nodes_nodes_create_builder, sasportal_nodes_nodes_nodes_create_task,
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
use crate::providers::gcp::clients::sasportal::SasportalCustomersDeploymentsMoveArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDeploymentsPatchArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDevicesCreateArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDevicesCreateSignedArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDevicesDeleteArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDevicesMoveArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDevicesPatchArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDevicesSignDeviceArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersDevicesUpdateSignedArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersMigrateOrganizationArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersNodesCreateArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersNodesDeleteArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersNodesDeploymentsCreateArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersNodesDevicesCreateArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersNodesDevicesCreateSignedArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersNodesMoveArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersNodesNodesCreateArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersNodesPatchArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersPatchArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersProvisionDeploymentArgs;
use crate::providers::gcp::clients::sasportal::SasportalCustomersSetupSasAnalyticsArgs;
use crate::providers::gcp::clients::sasportal::SasportalDeploymentsDevicesDeleteArgs;
use crate::providers::gcp::clients::sasportal::SasportalDeploymentsDevicesMoveArgs;
use crate::providers::gcp::clients::sasportal::SasportalDeploymentsDevicesPatchArgs;
use crate::providers::gcp::clients::sasportal::SasportalDeploymentsDevicesSignDeviceArgs;
use crate::providers::gcp::clients::sasportal::SasportalDeploymentsDevicesUpdateSignedArgs;
use crate::providers::gcp::clients::sasportal::SasportalInstallerGenerateSecretArgs;
use crate::providers::gcp::clients::sasportal::SasportalInstallerValidateArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDeploymentsDeleteArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDeploymentsDevicesCreateArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDeploymentsDevicesCreateSignedArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDeploymentsMoveArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDeploymentsPatchArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDevicesCreateArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDevicesCreateSignedArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDevicesDeleteArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDevicesMoveArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDevicesPatchArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDevicesSignDeviceArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesDevicesUpdateSignedArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesNodesCreateArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesNodesDeleteArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesNodesDeploymentsCreateArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesNodesDevicesCreateArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesNodesDevicesCreateSignedArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesNodesMoveArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesNodesNodesCreateArgs;
use crate::providers::gcp::clients::sasportal::SasportalNodesNodesPatchArgs;
use crate::providers::gcp::clients::sasportal::SasportalPoliciesGetArgs;
use crate::providers::gcp::clients::sasportal::SasportalPoliciesSetArgs;
use crate::providers::gcp::clients::sasportal::SasportalPoliciesTestArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// SasportalProvider with automatic state tracking.
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
/// let provider = SasportalProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct SasportalProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> SasportalProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new SasportalProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
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
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Sasportal policies get.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
