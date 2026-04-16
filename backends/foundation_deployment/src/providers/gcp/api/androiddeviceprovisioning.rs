//! AndroiddeviceprovisioningProvider - State-aware androiddeviceprovisioning API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       androiddeviceprovisioning API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::androiddeviceprovisioning::{
    androiddeviceprovisioning_customers_list_builder, androiddeviceprovisioning_customers_list_task,
    androiddeviceprovisioning_customers_configurations_create_builder, androiddeviceprovisioning_customers_configurations_create_task,
    androiddeviceprovisioning_customers_configurations_delete_builder, androiddeviceprovisioning_customers_configurations_delete_task,
    androiddeviceprovisioning_customers_configurations_get_builder, androiddeviceprovisioning_customers_configurations_get_task,
    androiddeviceprovisioning_customers_configurations_list_builder, androiddeviceprovisioning_customers_configurations_list_task,
    androiddeviceprovisioning_customers_configurations_patch_builder, androiddeviceprovisioning_customers_configurations_patch_task,
    androiddeviceprovisioning_customers_devices_apply_configuration_builder, androiddeviceprovisioning_customers_devices_apply_configuration_task,
    androiddeviceprovisioning_customers_devices_get_builder, androiddeviceprovisioning_customers_devices_get_task,
    androiddeviceprovisioning_customers_devices_list_builder, androiddeviceprovisioning_customers_devices_list_task,
    androiddeviceprovisioning_customers_devices_remove_configuration_builder, androiddeviceprovisioning_customers_devices_remove_configuration_task,
    androiddeviceprovisioning_customers_devices_unclaim_builder, androiddeviceprovisioning_customers_devices_unclaim_task,
    androiddeviceprovisioning_customers_dpcs_list_builder, androiddeviceprovisioning_customers_dpcs_list_task,
    androiddeviceprovisioning_operations_get_builder, androiddeviceprovisioning_operations_get_task,
    androiddeviceprovisioning_partners_customers_create_builder, androiddeviceprovisioning_partners_customers_create_task,
    androiddeviceprovisioning_partners_customers_list_builder, androiddeviceprovisioning_partners_customers_list_task,
    androiddeviceprovisioning_partners_devices_claim_builder, androiddeviceprovisioning_partners_devices_claim_task,
    androiddeviceprovisioning_partners_devices_claim_async_builder, androiddeviceprovisioning_partners_devices_claim_async_task,
    androiddeviceprovisioning_partners_devices_find_by_identifier_builder, androiddeviceprovisioning_partners_devices_find_by_identifier_task,
    androiddeviceprovisioning_partners_devices_find_by_owner_builder, androiddeviceprovisioning_partners_devices_find_by_owner_task,
    androiddeviceprovisioning_partners_devices_get_builder, androiddeviceprovisioning_partners_devices_get_task,
    androiddeviceprovisioning_partners_devices_get_sim_lock_state_builder, androiddeviceprovisioning_partners_devices_get_sim_lock_state_task,
    androiddeviceprovisioning_partners_devices_metadata_builder, androiddeviceprovisioning_partners_devices_metadata_task,
    androiddeviceprovisioning_partners_devices_unclaim_builder, androiddeviceprovisioning_partners_devices_unclaim_task,
    androiddeviceprovisioning_partners_devices_unclaim_async_builder, androiddeviceprovisioning_partners_devices_unclaim_async_task,
    androiddeviceprovisioning_partners_devices_update_metadata_async_builder, androiddeviceprovisioning_partners_devices_update_metadata_async_task,
    androiddeviceprovisioning_partners_vendors_list_builder, androiddeviceprovisioning_partners_vendors_list_task,
    androiddeviceprovisioning_partners_vendors_customers_list_builder, androiddeviceprovisioning_partners_vendors_customers_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::androiddeviceprovisioning::ClaimDeviceResponse;
use crate::providers::gcp::clients::androiddeviceprovisioning::Company;
use crate::providers::gcp::clients::androiddeviceprovisioning::Configuration;
use crate::providers::gcp::clients::androiddeviceprovisioning::CustomerListConfigurationsResponse;
use crate::providers::gcp::clients::androiddeviceprovisioning::CustomerListCustomersResponse;
use crate::providers::gcp::clients::androiddeviceprovisioning::CustomerListDevicesResponse;
use crate::providers::gcp::clients::androiddeviceprovisioning::CustomerListDpcsResponse;
use crate::providers::gcp::clients::androiddeviceprovisioning::Device;
use crate::providers::gcp::clients::androiddeviceprovisioning::DeviceMetadata;
use crate::providers::gcp::clients::androiddeviceprovisioning::Empty;
use crate::providers::gcp::clients::androiddeviceprovisioning::FindDevicesByDeviceIdentifierResponse;
use crate::providers::gcp::clients::androiddeviceprovisioning::FindDevicesByOwnerResponse;
use crate::providers::gcp::clients::androiddeviceprovisioning::GetDeviceSimLockStateResponse;
use crate::providers::gcp::clients::androiddeviceprovisioning::ListCustomersResponse;
use crate::providers::gcp::clients::androiddeviceprovisioning::ListVendorCustomersResponse;
use crate::providers::gcp::clients::androiddeviceprovisioning::ListVendorsResponse;
use crate::providers::gcp::clients::androiddeviceprovisioning::Operation;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningCustomersConfigurationsCreateArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningCustomersConfigurationsDeleteArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningCustomersConfigurationsGetArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningCustomersConfigurationsListArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningCustomersConfigurationsPatchArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningCustomersDevicesApplyConfigurationArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningCustomersDevicesGetArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningCustomersDevicesListArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningCustomersDevicesRemoveConfigurationArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningCustomersDevicesUnclaimArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningCustomersDpcsListArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningCustomersListArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningOperationsGetArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningPartnersCustomersCreateArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningPartnersCustomersListArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningPartnersDevicesClaimArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningPartnersDevicesClaimAsyncArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningPartnersDevicesFindByIdentifierArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningPartnersDevicesFindByOwnerArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningPartnersDevicesGetArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningPartnersDevicesGetSimLockStateArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningPartnersDevicesMetadataArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningPartnersDevicesUnclaimArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningPartnersDevicesUnclaimAsyncArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningPartnersDevicesUpdateMetadataAsyncArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningPartnersVendorsCustomersListArgs;
use crate::providers::gcp::clients::androiddeviceprovisioning::AndroiddeviceprovisioningPartnersVendorsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AndroiddeviceprovisioningProvider with automatic state tracking.
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
/// let provider = AndroiddeviceprovisioningProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct AndroiddeviceprovisioningProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> AndroiddeviceprovisioningProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new AndroiddeviceprovisioningProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new AndroiddeviceprovisioningProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Androiddeviceprovisioning customers list.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomerListCustomersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androiddeviceprovisioning_customers_list(
        &self,
        args: &AndroiddeviceprovisioningCustomersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomerListCustomersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_customers_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_customers_list_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning customers configurations create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Configuration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androiddeviceprovisioning_customers_configurations_create(
        &self,
        args: &AndroiddeviceprovisioningCustomersConfigurationsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Configuration, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_customers_configurations_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_customers_configurations_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning customers configurations delete.
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
    pub fn androiddeviceprovisioning_customers_configurations_delete(
        &self,
        args: &AndroiddeviceprovisioningCustomersConfigurationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_customers_configurations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_customers_configurations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning customers configurations get.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Configuration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androiddeviceprovisioning_customers_configurations_get(
        &self,
        args: &AndroiddeviceprovisioningCustomersConfigurationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Configuration, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_customers_configurations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_customers_configurations_get_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning customers configurations list.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomerListConfigurationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androiddeviceprovisioning_customers_configurations_list(
        &self,
        args: &AndroiddeviceprovisioningCustomersConfigurationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomerListConfigurationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_customers_configurations_list_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_customers_configurations_list_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning customers configurations patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Configuration result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androiddeviceprovisioning_customers_configurations_patch(
        &self,
        args: &AndroiddeviceprovisioningCustomersConfigurationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Configuration, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_customers_configurations_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_customers_configurations_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning customers devices apply configuration.
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
    pub fn androiddeviceprovisioning_customers_devices_apply_configuration(
        &self,
        args: &AndroiddeviceprovisioningCustomersDevicesApplyConfigurationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_customers_devices_apply_configuration_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_customers_devices_apply_configuration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning customers devices get.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Device result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androiddeviceprovisioning_customers_devices_get(
        &self,
        args: &AndroiddeviceprovisioningCustomersDevicesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Device, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_customers_devices_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_customers_devices_get_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning customers devices list.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomerListDevicesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androiddeviceprovisioning_customers_devices_list(
        &self,
        args: &AndroiddeviceprovisioningCustomersDevicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomerListDevicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_customers_devices_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_customers_devices_list_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning customers devices remove configuration.
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
    pub fn androiddeviceprovisioning_customers_devices_remove_configuration(
        &self,
        args: &AndroiddeviceprovisioningCustomersDevicesRemoveConfigurationArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_customers_devices_remove_configuration_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_customers_devices_remove_configuration_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning customers devices unclaim.
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
    pub fn androiddeviceprovisioning_customers_devices_unclaim(
        &self,
        args: &AndroiddeviceprovisioningCustomersDevicesUnclaimArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_customers_devices_unclaim_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_customers_devices_unclaim_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning customers dpcs list.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomerListDpcsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androiddeviceprovisioning_customers_dpcs_list(
        &self,
        args: &AndroiddeviceprovisioningCustomersDpcsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomerListDpcsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_customers_dpcs_list_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_customers_dpcs_list_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning operations get.
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
    pub fn androiddeviceprovisioning_operations_get(
        &self,
        args: &AndroiddeviceprovisioningOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning partners customers create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Company result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androiddeviceprovisioning_partners_customers_create(
        &self,
        args: &AndroiddeviceprovisioningPartnersCustomersCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Company, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_partners_customers_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_partners_customers_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning partners customers list.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListCustomersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androiddeviceprovisioning_partners_customers_list(
        &self,
        args: &AndroiddeviceprovisioningPartnersCustomersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListCustomersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_partners_customers_list_builder(
            &self.http_client,
            &args.partnerId,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_partners_customers_list_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning partners devices claim.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ClaimDeviceResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androiddeviceprovisioning_partners_devices_claim(
        &self,
        args: &AndroiddeviceprovisioningPartnersDevicesClaimArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ClaimDeviceResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_partners_devices_claim_builder(
            &self.http_client,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_partners_devices_claim_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning partners devices claim async.
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
    pub fn androiddeviceprovisioning_partners_devices_claim_async(
        &self,
        args: &AndroiddeviceprovisioningPartnersDevicesClaimAsyncArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_partners_devices_claim_async_builder(
            &self.http_client,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_partners_devices_claim_async_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning partners devices find by identifier.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FindDevicesByDeviceIdentifierResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androiddeviceprovisioning_partners_devices_find_by_identifier(
        &self,
        args: &AndroiddeviceprovisioningPartnersDevicesFindByIdentifierArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FindDevicesByDeviceIdentifierResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_partners_devices_find_by_identifier_builder(
            &self.http_client,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_partners_devices_find_by_identifier_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning partners devices find by owner.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FindDevicesByOwnerResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androiddeviceprovisioning_partners_devices_find_by_owner(
        &self,
        args: &AndroiddeviceprovisioningPartnersDevicesFindByOwnerArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FindDevicesByOwnerResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_partners_devices_find_by_owner_builder(
            &self.http_client,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_partners_devices_find_by_owner_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning partners devices get.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Device result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androiddeviceprovisioning_partners_devices_get(
        &self,
        args: &AndroiddeviceprovisioningPartnersDevicesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Device, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_partners_devices_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_partners_devices_get_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning partners devices get sim lock state.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GetDeviceSimLockStateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androiddeviceprovisioning_partners_devices_get_sim_lock_state(
        &self,
        args: &AndroiddeviceprovisioningPartnersDevicesGetSimLockStateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GetDeviceSimLockStateResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_partners_devices_get_sim_lock_state_builder(
            &self.http_client,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_partners_devices_get_sim_lock_state_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning partners devices metadata.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the DeviceMetadata result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androiddeviceprovisioning_partners_devices_metadata(
        &self,
        args: &AndroiddeviceprovisioningPartnersDevicesMetadataArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<DeviceMetadata, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_partners_devices_metadata_builder(
            &self.http_client,
            &args.metadataOwnerId,
            &args.deviceId,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_partners_devices_metadata_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning partners devices unclaim.
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
    pub fn androiddeviceprovisioning_partners_devices_unclaim(
        &self,
        args: &AndroiddeviceprovisioningPartnersDevicesUnclaimArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_partners_devices_unclaim_builder(
            &self.http_client,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_partners_devices_unclaim_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning partners devices unclaim async.
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
    pub fn androiddeviceprovisioning_partners_devices_unclaim_async(
        &self,
        args: &AndroiddeviceprovisioningPartnersDevicesUnclaimAsyncArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_partners_devices_unclaim_async_builder(
            &self.http_client,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_partners_devices_unclaim_async_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning partners devices update metadata async.
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
    pub fn androiddeviceprovisioning_partners_devices_update_metadata_async(
        &self,
        args: &AndroiddeviceprovisioningPartnersDevicesUpdateMetadataAsyncArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_partners_devices_update_metadata_async_builder(
            &self.http_client,
            &args.partnerId,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_partners_devices_update_metadata_async_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning partners vendors list.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListVendorsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androiddeviceprovisioning_partners_vendors_list(
        &self,
        args: &AndroiddeviceprovisioningPartnersVendorsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListVendorsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_partners_vendors_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_partners_vendors_list_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Androiddeviceprovisioning partners vendors customers list.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListVendorCustomersResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn androiddeviceprovisioning_partners_vendors_customers_list(
        &self,
        args: &AndroiddeviceprovisioningPartnersVendorsCustomersListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListVendorCustomersResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = androiddeviceprovisioning_partners_vendors_customers_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = androiddeviceprovisioning_partners_vendors_customers_list_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
