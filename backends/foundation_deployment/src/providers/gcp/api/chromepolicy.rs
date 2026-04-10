//! ChromepolicyProvider - State-aware chromepolicy API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       chromepolicy API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::chromepolicy::{
    chromepolicy_customers_policies_resolve_builder, chromepolicy_customers_policies_resolve_task,
    chromepolicy_customers_policies_groups_batch_delete_builder, chromepolicy_customers_policies_groups_batch_delete_task,
    chromepolicy_customers_policies_groups_batch_modify_builder, chromepolicy_customers_policies_groups_batch_modify_task,
    chromepolicy_customers_policies_groups_list_group_priority_ordering_builder, chromepolicy_customers_policies_groups_list_group_priority_ordering_task,
    chromepolicy_customers_policies_groups_update_group_priority_ordering_builder, chromepolicy_customers_policies_groups_update_group_priority_ordering_task,
    chromepolicy_customers_policies_networks_define_certificate_builder, chromepolicy_customers_policies_networks_define_certificate_task,
    chromepolicy_customers_policies_networks_define_network_builder, chromepolicy_customers_policies_networks_define_network_task,
    chromepolicy_customers_policies_networks_remove_certificate_builder, chromepolicy_customers_policies_networks_remove_certificate_task,
    chromepolicy_customers_policies_networks_remove_network_builder, chromepolicy_customers_policies_networks_remove_network_task,
    chromepolicy_customers_policies_orgunits_batch_inherit_builder, chromepolicy_customers_policies_orgunits_batch_inherit_task,
    chromepolicy_customers_policies_orgunits_batch_modify_builder, chromepolicy_customers_policies_orgunits_batch_modify_task,
    chromepolicy_media_upload_builder, chromepolicy_media_upload_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::chromepolicy::GoogleChromePolicyVersionsV1DefineCertificateResponse;
use crate::providers::gcp::clients::chromepolicy::GoogleChromePolicyVersionsV1DefineNetworkResponse;
use crate::providers::gcp::clients::chromepolicy::GoogleChromePolicyVersionsV1ListGroupPriorityOrderingResponse;
use crate::providers::gcp::clients::chromepolicy::GoogleChromePolicyVersionsV1RemoveCertificateResponse;
use crate::providers::gcp::clients::chromepolicy::GoogleChromePolicyVersionsV1RemoveNetworkResponse;
use crate::providers::gcp::clients::chromepolicy::GoogleChromePolicyVersionsV1ResolveResponse;
use crate::providers::gcp::clients::chromepolicy::GoogleChromePolicyVersionsV1UploadPolicyFileResponse;
use crate::providers::gcp::clients::chromepolicy::GoogleProtobufEmpty;
use crate::providers::gcp::clients::chromepolicy::ChromepolicyCustomersPoliciesGroupsBatchDeleteArgs;
use crate::providers::gcp::clients::chromepolicy::ChromepolicyCustomersPoliciesGroupsBatchModifyArgs;
use crate::providers::gcp::clients::chromepolicy::ChromepolicyCustomersPoliciesGroupsListGroupPriorityOrderingArgs;
use crate::providers::gcp::clients::chromepolicy::ChromepolicyCustomersPoliciesGroupsUpdateGroupPriorityOrderingArgs;
use crate::providers::gcp::clients::chromepolicy::ChromepolicyCustomersPoliciesNetworksDefineCertificateArgs;
use crate::providers::gcp::clients::chromepolicy::ChromepolicyCustomersPoliciesNetworksDefineNetworkArgs;
use crate::providers::gcp::clients::chromepolicy::ChromepolicyCustomersPoliciesNetworksRemoveCertificateArgs;
use crate::providers::gcp::clients::chromepolicy::ChromepolicyCustomersPoliciesNetworksRemoveNetworkArgs;
use crate::providers::gcp::clients::chromepolicy::ChromepolicyCustomersPoliciesOrgunitsBatchInheritArgs;
use crate::providers::gcp::clients::chromepolicy::ChromepolicyCustomersPoliciesOrgunitsBatchModifyArgs;
use crate::providers::gcp::clients::chromepolicy::ChromepolicyCustomersPoliciesResolveArgs;
use crate::providers::gcp::clients::chromepolicy::ChromepolicyMediaUploadArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ChromepolicyProvider with automatic state tracking.
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
/// let provider = ChromepolicyProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ChromepolicyProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ChromepolicyProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ChromepolicyProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Chromepolicy customers policies resolve.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromePolicyVersionsV1ResolveResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromepolicy_customers_policies_resolve(
        &self,
        args: &ChromepolicyCustomersPoliciesResolveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromePolicyVersionsV1ResolveResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromepolicy_customers_policies_resolve_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = chromepolicy_customers_policies_resolve_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromepolicy customers policies groups batch delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromepolicy_customers_policies_groups_batch_delete(
        &self,
        args: &ChromepolicyCustomersPoliciesGroupsBatchDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromepolicy_customers_policies_groups_batch_delete_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = chromepolicy_customers_policies_groups_batch_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromepolicy customers policies groups batch modify.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromepolicy_customers_policies_groups_batch_modify(
        &self,
        args: &ChromepolicyCustomersPoliciesGroupsBatchModifyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromepolicy_customers_policies_groups_batch_modify_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = chromepolicy_customers_policies_groups_batch_modify_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromepolicy customers policies groups list group priority ordering.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromePolicyVersionsV1ListGroupPriorityOrderingResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromepolicy_customers_policies_groups_list_group_priority_ordering(
        &self,
        args: &ChromepolicyCustomersPoliciesGroupsListGroupPriorityOrderingArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromePolicyVersionsV1ListGroupPriorityOrderingResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromepolicy_customers_policies_groups_list_group_priority_ordering_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = chromepolicy_customers_policies_groups_list_group_priority_ordering_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromepolicy customers policies groups update group priority ordering.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromepolicy_customers_policies_groups_update_group_priority_ordering(
        &self,
        args: &ChromepolicyCustomersPoliciesGroupsUpdateGroupPriorityOrderingArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromepolicy_customers_policies_groups_update_group_priority_ordering_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = chromepolicy_customers_policies_groups_update_group_priority_ordering_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromepolicy customers policies networks define certificate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromePolicyVersionsV1DefineCertificateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromepolicy_customers_policies_networks_define_certificate(
        &self,
        args: &ChromepolicyCustomersPoliciesNetworksDefineCertificateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromePolicyVersionsV1DefineCertificateResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromepolicy_customers_policies_networks_define_certificate_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = chromepolicy_customers_policies_networks_define_certificate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromepolicy customers policies networks define network.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromePolicyVersionsV1DefineNetworkResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromepolicy_customers_policies_networks_define_network(
        &self,
        args: &ChromepolicyCustomersPoliciesNetworksDefineNetworkArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromePolicyVersionsV1DefineNetworkResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromepolicy_customers_policies_networks_define_network_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = chromepolicy_customers_policies_networks_define_network_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromepolicy customers policies networks remove certificate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromePolicyVersionsV1RemoveCertificateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromepolicy_customers_policies_networks_remove_certificate(
        &self,
        args: &ChromepolicyCustomersPoliciesNetworksRemoveCertificateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromePolicyVersionsV1RemoveCertificateResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromepolicy_customers_policies_networks_remove_certificate_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = chromepolicy_customers_policies_networks_remove_certificate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromepolicy customers policies networks remove network.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromePolicyVersionsV1RemoveNetworkResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromepolicy_customers_policies_networks_remove_network(
        &self,
        args: &ChromepolicyCustomersPoliciesNetworksRemoveNetworkArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromePolicyVersionsV1RemoveNetworkResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromepolicy_customers_policies_networks_remove_network_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = chromepolicy_customers_policies_networks_remove_network_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromepolicy customers policies orgunits batch inherit.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromepolicy_customers_policies_orgunits_batch_inherit(
        &self,
        args: &ChromepolicyCustomersPoliciesOrgunitsBatchInheritArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromepolicy_customers_policies_orgunits_batch_inherit_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = chromepolicy_customers_policies_orgunits_batch_inherit_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromepolicy customers policies orgunits batch modify.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleProtobufEmpty result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromepolicy_customers_policies_orgunits_batch_modify(
        &self,
        args: &ChromepolicyCustomersPoliciesOrgunitsBatchModifyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromepolicy_customers_policies_orgunits_batch_modify_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = chromepolicy_customers_policies_orgunits_batch_modify_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Chromepolicy media upload.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChromePolicyVersionsV1UploadPolicyFileResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn chromepolicy_media_upload(
        &self,
        args: &ChromepolicyMediaUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChromePolicyVersionsV1UploadPolicyFileResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = chromepolicy_media_upload_builder(
            &self.http_client,
            &args.customer,
        )
        .map_err(ProviderError::Api)?;

        let task = chromepolicy_media_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
