//! DnsProvider - State-aware dns API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       dns API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::dns::{
    dns_changes_create_builder, dns_changes_create_task,
    dns_managed_zones_create_builder, dns_managed_zones_create_task,
    dns_managed_zones_delete_builder, dns_managed_zones_delete_task,
    dns_managed_zones_get_iam_policy_builder, dns_managed_zones_get_iam_policy_task,
    dns_managed_zones_patch_builder, dns_managed_zones_patch_task,
    dns_managed_zones_set_iam_policy_builder, dns_managed_zones_set_iam_policy_task,
    dns_managed_zones_test_iam_permissions_builder, dns_managed_zones_test_iam_permissions_task,
    dns_managed_zones_update_builder, dns_managed_zones_update_task,
    dns_policies_create_builder, dns_policies_create_task,
    dns_policies_delete_builder, dns_policies_delete_task,
    dns_policies_patch_builder, dns_policies_patch_task,
    dns_policies_update_builder, dns_policies_update_task,
    dns_resource_record_sets_create_builder, dns_resource_record_sets_create_task,
    dns_resource_record_sets_delete_builder, dns_resource_record_sets_delete_task,
    dns_resource_record_sets_patch_builder, dns_resource_record_sets_patch_task,
    dns_response_policies_create_builder, dns_response_policies_create_task,
    dns_response_policies_delete_builder, dns_response_policies_delete_task,
    dns_response_policies_patch_builder, dns_response_policies_patch_task,
    dns_response_policies_update_builder, dns_response_policies_update_task,
    dns_response_policy_rules_create_builder, dns_response_policy_rules_create_task,
    dns_response_policy_rules_delete_builder, dns_response_policy_rules_delete_task,
    dns_response_policy_rules_patch_builder, dns_response_policy_rules_patch_task,
    dns_response_policy_rules_update_builder, dns_response_policy_rules_update_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::dns::Change;
use crate::providers::gcp::clients::dns::GoogleIamV1Policy;
use crate::providers::gcp::clients::dns::GoogleIamV1TestIamPermissionsResponse;
use crate::providers::gcp::clients::dns::ManagedZone;
use crate::providers::gcp::clients::dns::Operation;
use crate::providers::gcp::clients::dns::PoliciesPatchResponse;
use crate::providers::gcp::clients::dns::PoliciesUpdateResponse;
use crate::providers::gcp::clients::dns::Policy;
use crate::providers::gcp::clients::dns::ResourceRecordSet;
use crate::providers::gcp::clients::dns::ResourceRecordSetsDeleteResponse;
use crate::providers::gcp::clients::dns::ResponsePoliciesPatchResponse;
use crate::providers::gcp::clients::dns::ResponsePoliciesUpdateResponse;
use crate::providers::gcp::clients::dns::ResponsePolicy;
use crate::providers::gcp::clients::dns::ResponsePolicyRule;
use crate::providers::gcp::clients::dns::ResponsePolicyRulesPatchResponse;
use crate::providers::gcp::clients::dns::ResponsePolicyRulesUpdateResponse;
use crate::providers::gcp::clients::dns::DnsChangesCreateArgs;
use crate::providers::gcp::clients::dns::DnsManagedZonesCreateArgs;
use crate::providers::gcp::clients::dns::DnsManagedZonesDeleteArgs;
use crate::providers::gcp::clients::dns::DnsManagedZonesGetIamPolicyArgs;
use crate::providers::gcp::clients::dns::DnsManagedZonesPatchArgs;
use crate::providers::gcp::clients::dns::DnsManagedZonesSetIamPolicyArgs;
use crate::providers::gcp::clients::dns::DnsManagedZonesTestIamPermissionsArgs;
use crate::providers::gcp::clients::dns::DnsManagedZonesUpdateArgs;
use crate::providers::gcp::clients::dns::DnsPoliciesCreateArgs;
use crate::providers::gcp::clients::dns::DnsPoliciesDeleteArgs;
use crate::providers::gcp::clients::dns::DnsPoliciesPatchArgs;
use crate::providers::gcp::clients::dns::DnsPoliciesUpdateArgs;
use crate::providers::gcp::clients::dns::DnsResourceRecordSetsCreateArgs;
use crate::providers::gcp::clients::dns::DnsResourceRecordSetsDeleteArgs;
use crate::providers::gcp::clients::dns::DnsResourceRecordSetsPatchArgs;
use crate::providers::gcp::clients::dns::DnsResponsePoliciesCreateArgs;
use crate::providers::gcp::clients::dns::DnsResponsePoliciesDeleteArgs;
use crate::providers::gcp::clients::dns::DnsResponsePoliciesPatchArgs;
use crate::providers::gcp::clients::dns::DnsResponsePoliciesUpdateArgs;
use crate::providers::gcp::clients::dns::DnsResponsePolicyRulesCreateArgs;
use crate::providers::gcp::clients::dns::DnsResponsePolicyRulesDeleteArgs;
use crate::providers::gcp::clients::dns::DnsResponsePolicyRulesPatchArgs;
use crate::providers::gcp::clients::dns::DnsResponsePolicyRulesUpdateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// DnsProvider with automatic state tracking.
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
/// let provider = DnsProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct DnsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> DnsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new DnsProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Dns changes create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Change result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dns_changes_create(
        &self,
        args: &DnsChangesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Change, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dns_changes_create_builder(
            &self.http_client,
            &args.project,
            &args.managedZone,
            &args.clientOperationId,
        )
        .map_err(ProviderError::Api)?;

        let task = dns_changes_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dns managed zones create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ManagedZone result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dns_managed_zones_create(
        &self,
        args: &DnsManagedZonesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ManagedZone, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dns_managed_zones_create_builder(
            &self.http_client,
            &args.project,
            &args.clientOperationId,
        )
        .map_err(ProviderError::Api)?;

        let task = dns_managed_zones_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dns managed zones delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dns_managed_zones_delete(
        &self,
        args: &DnsManagedZonesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dns_managed_zones_delete_builder(
            &self.http_client,
            &args.project,
            &args.managedZone,
            &args.clientOperationId,
        )
        .map_err(ProviderError::Api)?;

        let task = dns_managed_zones_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dns managed zones get iam policy.
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
    pub fn dns_managed_zones_get_iam_policy(
        &self,
        args: &DnsManagedZonesGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dns_managed_zones_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dns_managed_zones_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dns managed zones patch.
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
    pub fn dns_managed_zones_patch(
        &self,
        args: &DnsManagedZonesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dns_managed_zones_patch_builder(
            &self.http_client,
            &args.project,
            &args.managedZone,
            &args.clientOperationId,
        )
        .map_err(ProviderError::Api)?;

        let task = dns_managed_zones_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dns managed zones set iam policy.
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
    pub fn dns_managed_zones_set_iam_policy(
        &self,
        args: &DnsManagedZonesSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dns_managed_zones_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dns_managed_zones_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dns managed zones test iam permissions.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dns_managed_zones_test_iam_permissions(
        &self,
        args: &DnsManagedZonesTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleIamV1TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dns_managed_zones_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = dns_managed_zones_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dns managed zones update.
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
    pub fn dns_managed_zones_update(
        &self,
        args: &DnsManagedZonesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dns_managed_zones_update_builder(
            &self.http_client,
            &args.project,
            &args.managedZone,
            &args.clientOperationId,
        )
        .map_err(ProviderError::Api)?;

        let task = dns_managed_zones_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dns policies create.
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
    pub fn dns_policies_create(
        &self,
        args: &DnsPoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dns_policies_create_builder(
            &self.http_client,
            &args.project,
            &args.clientOperationId,
        )
        .map_err(ProviderError::Api)?;

        let task = dns_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dns policies delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dns_policies_delete(
        &self,
        args: &DnsPoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dns_policies_delete_builder(
            &self.http_client,
            &args.project,
            &args.policy,
            &args.clientOperationId,
        )
        .map_err(ProviderError::Api)?;

        let task = dns_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dns policies patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PoliciesPatchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dns_policies_patch(
        &self,
        args: &DnsPoliciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PoliciesPatchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dns_policies_patch_builder(
            &self.http_client,
            &args.project,
            &args.policy,
            &args.clientOperationId,
        )
        .map_err(ProviderError::Api)?;

        let task = dns_policies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dns policies update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the PoliciesUpdateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dns_policies_update(
        &self,
        args: &DnsPoliciesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<PoliciesUpdateResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dns_policies_update_builder(
            &self.http_client,
            &args.project,
            &args.policy,
            &args.clientOperationId,
        )
        .map_err(ProviderError::Api)?;

        let task = dns_policies_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dns resource record sets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ResourceRecordSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dns_resource_record_sets_create(
        &self,
        args: &DnsResourceRecordSetsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ResourceRecordSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dns_resource_record_sets_create_builder(
            &self.http_client,
            &args.project,
            &args.managedZone,
            &args.clientOperationId,
        )
        .map_err(ProviderError::Api)?;

        let task = dns_resource_record_sets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dns resource record sets delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ResourceRecordSetsDeleteResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dns_resource_record_sets_delete(
        &self,
        args: &DnsResourceRecordSetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ResourceRecordSetsDeleteResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dns_resource_record_sets_delete_builder(
            &self.http_client,
            &args.project,
            &args.managedZone,
            &args.name,
            &args.type,
            &args.clientOperationId,
        )
        .map_err(ProviderError::Api)?;

        let task = dns_resource_record_sets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dns resource record sets patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ResourceRecordSet result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dns_resource_record_sets_patch(
        &self,
        args: &DnsResourceRecordSetsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ResourceRecordSet, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dns_resource_record_sets_patch_builder(
            &self.http_client,
            &args.project,
            &args.managedZone,
            &args.name,
            &args.type,
            &args.clientOperationId,
        )
        .map_err(ProviderError::Api)?;

        let task = dns_resource_record_sets_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dns response policies create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ResponsePolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dns_response_policies_create(
        &self,
        args: &DnsResponsePoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ResponsePolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dns_response_policies_create_builder(
            &self.http_client,
            &args.project,
            &args.clientOperationId,
        )
        .map_err(ProviderError::Api)?;

        let task = dns_response_policies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dns response policies delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dns_response_policies_delete(
        &self,
        args: &DnsResponsePoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dns_response_policies_delete_builder(
            &self.http_client,
            &args.project,
            &args.responsePolicy,
            &args.clientOperationId,
        )
        .map_err(ProviderError::Api)?;

        let task = dns_response_policies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dns response policies patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ResponsePoliciesPatchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dns_response_policies_patch(
        &self,
        args: &DnsResponsePoliciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ResponsePoliciesPatchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dns_response_policies_patch_builder(
            &self.http_client,
            &args.project,
            &args.responsePolicy,
            &args.clientOperationId,
        )
        .map_err(ProviderError::Api)?;

        let task = dns_response_policies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dns response policies update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ResponsePoliciesUpdateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dns_response_policies_update(
        &self,
        args: &DnsResponsePoliciesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ResponsePoliciesUpdateResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dns_response_policies_update_builder(
            &self.http_client,
            &args.project,
            &args.responsePolicy,
            &args.clientOperationId,
        )
        .map_err(ProviderError::Api)?;

        let task = dns_response_policies_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dns response policy rules create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ResponsePolicyRule result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dns_response_policy_rules_create(
        &self,
        args: &DnsResponsePolicyRulesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ResponsePolicyRule, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dns_response_policy_rules_create_builder(
            &self.http_client,
            &args.project,
            &args.responsePolicy,
            &args.clientOperationId,
        )
        .map_err(ProviderError::Api)?;

        let task = dns_response_policy_rules_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dns response policy rules delete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the serde_json::Value result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dns_response_policy_rules_delete(
        &self,
        args: &DnsResponsePolicyRulesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dns_response_policy_rules_delete_builder(
            &self.http_client,
            &args.project,
            &args.responsePolicy,
            &args.responsePolicyRule,
            &args.clientOperationId,
        )
        .map_err(ProviderError::Api)?;

        let task = dns_response_policy_rules_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dns response policy rules patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ResponsePolicyRulesPatchResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dns_response_policy_rules_patch(
        &self,
        args: &DnsResponsePolicyRulesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ResponsePolicyRulesPatchResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dns_response_policy_rules_patch_builder(
            &self.http_client,
            &args.project,
            &args.responsePolicy,
            &args.responsePolicyRule,
            &args.clientOperationId,
        )
        .map_err(ProviderError::Api)?;

        let task = dns_response_policy_rules_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Dns response policy rules update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ResponsePolicyRulesUpdateResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn dns_response_policy_rules_update(
        &self,
        args: &DnsResponsePolicyRulesUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ResponsePolicyRulesUpdateResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = dns_response_policy_rules_update_builder(
            &self.http_client,
            &args.project,
            &args.responsePolicy,
            &args.responsePolicyRule,
            &args.clientOperationId,
        )
        .map_err(ProviderError::Api)?;

        let task = dns_response_policy_rules_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
