//! RecaptchaenterpriseProvider - State-aware recaptchaenterprise API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       recaptchaenterprise API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::recaptchaenterprise::{
    recaptchaenterprise_projects_assessments_annotate_builder, recaptchaenterprise_projects_assessments_annotate_task,
    recaptchaenterprise_projects_assessments_create_builder, recaptchaenterprise_projects_assessments_create_task,
    recaptchaenterprise_projects_firewallpolicies_create_builder, recaptchaenterprise_projects_firewallpolicies_create_task,
    recaptchaenterprise_projects_firewallpolicies_delete_builder, recaptchaenterprise_projects_firewallpolicies_delete_task,
    recaptchaenterprise_projects_firewallpolicies_get_builder, recaptchaenterprise_projects_firewallpolicies_get_task,
    recaptchaenterprise_projects_firewallpolicies_list_builder, recaptchaenterprise_projects_firewallpolicies_list_task,
    recaptchaenterprise_projects_firewallpolicies_patch_builder, recaptchaenterprise_projects_firewallpolicies_patch_task,
    recaptchaenterprise_projects_firewallpolicies_reorder_builder, recaptchaenterprise_projects_firewallpolicies_reorder_task,
    recaptchaenterprise_projects_keys_add_ip_override_builder, recaptchaenterprise_projects_keys_add_ip_override_task,
    recaptchaenterprise_projects_keys_create_builder, recaptchaenterprise_projects_keys_create_task,
    recaptchaenterprise_projects_keys_delete_builder, recaptchaenterprise_projects_keys_delete_task,
    recaptchaenterprise_projects_keys_get_builder, recaptchaenterprise_projects_keys_get_task,
    recaptchaenterprise_projects_keys_get_metrics_builder, recaptchaenterprise_projects_keys_get_metrics_task,
    recaptchaenterprise_projects_keys_list_builder, recaptchaenterprise_projects_keys_list_task,
    recaptchaenterprise_projects_keys_list_ip_overrides_builder, recaptchaenterprise_projects_keys_list_ip_overrides_task,
    recaptchaenterprise_projects_keys_migrate_builder, recaptchaenterprise_projects_keys_migrate_task,
    recaptchaenterprise_projects_keys_patch_builder, recaptchaenterprise_projects_keys_patch_task,
    recaptchaenterprise_projects_keys_remove_ip_override_builder, recaptchaenterprise_projects_keys_remove_ip_override_task,
    recaptchaenterprise_projects_keys_retrieve_legacy_secret_key_builder, recaptchaenterprise_projects_keys_retrieve_legacy_secret_key_task,
    recaptchaenterprise_projects_relatedaccountgroupmemberships_search_builder, recaptchaenterprise_projects_relatedaccountgroupmemberships_search_task,
    recaptchaenterprise_projects_relatedaccountgroups_list_builder, recaptchaenterprise_projects_relatedaccountgroups_list_task,
    recaptchaenterprise_projects_relatedaccountgroups_memberships_list_builder, recaptchaenterprise_projects_relatedaccountgroups_memberships_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::recaptchaenterprise::GoogleCloudRecaptchaenterpriseV1AddIpOverrideResponse;
use crate::providers::gcp::clients::recaptchaenterprise::GoogleCloudRecaptchaenterpriseV1AnnotateAssessmentResponse;
use crate::providers::gcp::clients::recaptchaenterprise::GoogleCloudRecaptchaenterpriseV1Assessment;
use crate::providers::gcp::clients::recaptchaenterprise::GoogleCloudRecaptchaenterpriseV1FirewallPolicy;
use crate::providers::gcp::clients::recaptchaenterprise::GoogleCloudRecaptchaenterpriseV1Key;
use crate::providers::gcp::clients::recaptchaenterprise::GoogleCloudRecaptchaenterpriseV1ListFirewallPoliciesResponse;
use crate::providers::gcp::clients::recaptchaenterprise::GoogleCloudRecaptchaenterpriseV1ListIpOverridesResponse;
use crate::providers::gcp::clients::recaptchaenterprise::GoogleCloudRecaptchaenterpriseV1ListKeysResponse;
use crate::providers::gcp::clients::recaptchaenterprise::GoogleCloudRecaptchaenterpriseV1ListRelatedAccountGroupMembershipsResponse;
use crate::providers::gcp::clients::recaptchaenterprise::GoogleCloudRecaptchaenterpriseV1ListRelatedAccountGroupsResponse;
use crate::providers::gcp::clients::recaptchaenterprise::GoogleCloudRecaptchaenterpriseV1Metrics;
use crate::providers::gcp::clients::recaptchaenterprise::GoogleCloudRecaptchaenterpriseV1RemoveIpOverrideResponse;
use crate::providers::gcp::clients::recaptchaenterprise::GoogleCloudRecaptchaenterpriseV1ReorderFirewallPoliciesResponse;
use crate::providers::gcp::clients::recaptchaenterprise::GoogleCloudRecaptchaenterpriseV1RetrieveLegacySecretKeyResponse;
use crate::providers::gcp::clients::recaptchaenterprise::GoogleCloudRecaptchaenterpriseV1SearchRelatedAccountGroupMembershipsResponse;
use crate::providers::gcp::clients::recaptchaenterprise::GoogleProtobufEmpty;
use crate::providers::gcp::clients::recaptchaenterprise::RecaptchaenterpriseProjectsAssessmentsAnnotateArgs;
use crate::providers::gcp::clients::recaptchaenterprise::RecaptchaenterpriseProjectsAssessmentsCreateArgs;
use crate::providers::gcp::clients::recaptchaenterprise::RecaptchaenterpriseProjectsFirewallpoliciesCreateArgs;
use crate::providers::gcp::clients::recaptchaenterprise::RecaptchaenterpriseProjectsFirewallpoliciesDeleteArgs;
use crate::providers::gcp::clients::recaptchaenterprise::RecaptchaenterpriseProjectsFirewallpoliciesGetArgs;
use crate::providers::gcp::clients::recaptchaenterprise::RecaptchaenterpriseProjectsFirewallpoliciesListArgs;
use crate::providers::gcp::clients::recaptchaenterprise::RecaptchaenterpriseProjectsFirewallpoliciesPatchArgs;
use crate::providers::gcp::clients::recaptchaenterprise::RecaptchaenterpriseProjectsFirewallpoliciesReorderArgs;
use crate::providers::gcp::clients::recaptchaenterprise::RecaptchaenterpriseProjectsKeysAddIpOverrideArgs;
use crate::providers::gcp::clients::recaptchaenterprise::RecaptchaenterpriseProjectsKeysCreateArgs;
use crate::providers::gcp::clients::recaptchaenterprise::RecaptchaenterpriseProjectsKeysDeleteArgs;
use crate::providers::gcp::clients::recaptchaenterprise::RecaptchaenterpriseProjectsKeysGetArgs;
use crate::providers::gcp::clients::recaptchaenterprise::RecaptchaenterpriseProjectsKeysGetMetricsArgs;
use crate::providers::gcp::clients::recaptchaenterprise::RecaptchaenterpriseProjectsKeysListArgs;
use crate::providers::gcp::clients::recaptchaenterprise::RecaptchaenterpriseProjectsKeysListIpOverridesArgs;
use crate::providers::gcp::clients::recaptchaenterprise::RecaptchaenterpriseProjectsKeysMigrateArgs;
use crate::providers::gcp::clients::recaptchaenterprise::RecaptchaenterpriseProjectsKeysPatchArgs;
use crate::providers::gcp::clients::recaptchaenterprise::RecaptchaenterpriseProjectsKeysRemoveIpOverrideArgs;
use crate::providers::gcp::clients::recaptchaenterprise::RecaptchaenterpriseProjectsKeysRetrieveLegacySecretKeyArgs;
use crate::providers::gcp::clients::recaptchaenterprise::RecaptchaenterpriseProjectsRelatedaccountgroupmembershipsSearchArgs;
use crate::providers::gcp::clients::recaptchaenterprise::RecaptchaenterpriseProjectsRelatedaccountgroupsListArgs;
use crate::providers::gcp::clients::recaptchaenterprise::RecaptchaenterpriseProjectsRelatedaccountgroupsMembershipsListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// RecaptchaenterpriseProvider with automatic state tracking.
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
/// let provider = RecaptchaenterpriseProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct RecaptchaenterpriseProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> RecaptchaenterpriseProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new RecaptchaenterpriseProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new RecaptchaenterpriseProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Recaptchaenterprise projects assessments annotate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecaptchaenterpriseV1AnnotateAssessmentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recaptchaenterprise_projects_assessments_annotate(
        &self,
        args: &RecaptchaenterpriseProjectsAssessmentsAnnotateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecaptchaenterpriseV1AnnotateAssessmentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recaptchaenterprise_projects_assessments_annotate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recaptchaenterprise_projects_assessments_annotate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recaptchaenterprise projects assessments create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecaptchaenterpriseV1Assessment result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recaptchaenterprise_projects_assessments_create(
        &self,
        args: &RecaptchaenterpriseProjectsAssessmentsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecaptchaenterpriseV1Assessment, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recaptchaenterprise_projects_assessments_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = recaptchaenterprise_projects_assessments_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recaptchaenterprise projects firewallpolicies create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecaptchaenterpriseV1FirewallPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recaptchaenterprise_projects_firewallpolicies_create(
        &self,
        args: &RecaptchaenterpriseProjectsFirewallpoliciesCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecaptchaenterpriseV1FirewallPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recaptchaenterprise_projects_firewallpolicies_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = recaptchaenterprise_projects_firewallpolicies_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recaptchaenterprise projects firewallpolicies delete.
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
    pub fn recaptchaenterprise_projects_firewallpolicies_delete(
        &self,
        args: &RecaptchaenterpriseProjectsFirewallpoliciesDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recaptchaenterprise_projects_firewallpolicies_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recaptchaenterprise_projects_firewallpolicies_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recaptchaenterprise projects firewallpolicies get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecaptchaenterpriseV1FirewallPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn recaptchaenterprise_projects_firewallpolicies_get(
        &self,
        args: &RecaptchaenterpriseProjectsFirewallpoliciesGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecaptchaenterpriseV1FirewallPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recaptchaenterprise_projects_firewallpolicies_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recaptchaenterprise_projects_firewallpolicies_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recaptchaenterprise projects firewallpolicies list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecaptchaenterpriseV1ListFirewallPoliciesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn recaptchaenterprise_projects_firewallpolicies_list(
        &self,
        args: &RecaptchaenterpriseProjectsFirewallpoliciesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecaptchaenterpriseV1ListFirewallPoliciesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recaptchaenterprise_projects_firewallpolicies_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = recaptchaenterprise_projects_firewallpolicies_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recaptchaenterprise projects firewallpolicies patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecaptchaenterpriseV1FirewallPolicy result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recaptchaenterprise_projects_firewallpolicies_patch(
        &self,
        args: &RecaptchaenterpriseProjectsFirewallpoliciesPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecaptchaenterpriseV1FirewallPolicy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recaptchaenterprise_projects_firewallpolicies_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = recaptchaenterprise_projects_firewallpolicies_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recaptchaenterprise projects firewallpolicies reorder.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecaptchaenterpriseV1ReorderFirewallPoliciesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recaptchaenterprise_projects_firewallpolicies_reorder(
        &self,
        args: &RecaptchaenterpriseProjectsFirewallpoliciesReorderArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecaptchaenterpriseV1ReorderFirewallPoliciesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recaptchaenterprise_projects_firewallpolicies_reorder_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = recaptchaenterprise_projects_firewallpolicies_reorder_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recaptchaenterprise projects keys add ip override.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecaptchaenterpriseV1AddIpOverrideResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recaptchaenterprise_projects_keys_add_ip_override(
        &self,
        args: &RecaptchaenterpriseProjectsKeysAddIpOverrideArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecaptchaenterpriseV1AddIpOverrideResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recaptchaenterprise_projects_keys_add_ip_override_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recaptchaenterprise_projects_keys_add_ip_override_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recaptchaenterprise projects keys create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecaptchaenterpriseV1Key result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recaptchaenterprise_projects_keys_create(
        &self,
        args: &RecaptchaenterpriseProjectsKeysCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecaptchaenterpriseV1Key, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recaptchaenterprise_projects_keys_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = recaptchaenterprise_projects_keys_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recaptchaenterprise projects keys delete.
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
    pub fn recaptchaenterprise_projects_keys_delete(
        &self,
        args: &RecaptchaenterpriseProjectsKeysDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recaptchaenterprise_projects_keys_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recaptchaenterprise_projects_keys_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recaptchaenterprise projects keys get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecaptchaenterpriseV1Key result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn recaptchaenterprise_projects_keys_get(
        &self,
        args: &RecaptchaenterpriseProjectsKeysGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecaptchaenterpriseV1Key, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recaptchaenterprise_projects_keys_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recaptchaenterprise_projects_keys_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recaptchaenterprise projects keys get metrics.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecaptchaenterpriseV1Metrics result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn recaptchaenterprise_projects_keys_get_metrics(
        &self,
        args: &RecaptchaenterpriseProjectsKeysGetMetricsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecaptchaenterpriseV1Metrics, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recaptchaenterprise_projects_keys_get_metrics_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recaptchaenterprise_projects_keys_get_metrics_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recaptchaenterprise projects keys list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecaptchaenterpriseV1ListKeysResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn recaptchaenterprise_projects_keys_list(
        &self,
        args: &RecaptchaenterpriseProjectsKeysListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecaptchaenterpriseV1ListKeysResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recaptchaenterprise_projects_keys_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = recaptchaenterprise_projects_keys_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recaptchaenterprise projects keys list ip overrides.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecaptchaenterpriseV1ListIpOverridesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn recaptchaenterprise_projects_keys_list_ip_overrides(
        &self,
        args: &RecaptchaenterpriseProjectsKeysListIpOverridesArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecaptchaenterpriseV1ListIpOverridesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recaptchaenterprise_projects_keys_list_ip_overrides_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = recaptchaenterprise_projects_keys_list_ip_overrides_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recaptchaenterprise projects keys migrate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecaptchaenterpriseV1Key result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recaptchaenterprise_projects_keys_migrate(
        &self,
        args: &RecaptchaenterpriseProjectsKeysMigrateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecaptchaenterpriseV1Key, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recaptchaenterprise_projects_keys_migrate_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recaptchaenterprise_projects_keys_migrate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recaptchaenterprise projects keys patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecaptchaenterpriseV1Key result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recaptchaenterprise_projects_keys_patch(
        &self,
        args: &RecaptchaenterpriseProjectsKeysPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecaptchaenterpriseV1Key, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recaptchaenterprise_projects_keys_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = recaptchaenterprise_projects_keys_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recaptchaenterprise projects keys remove ip override.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecaptchaenterpriseV1RemoveIpOverrideResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn recaptchaenterprise_projects_keys_remove_ip_override(
        &self,
        args: &RecaptchaenterpriseProjectsKeysRemoveIpOverrideArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecaptchaenterpriseV1RemoveIpOverrideResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recaptchaenterprise_projects_keys_remove_ip_override_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = recaptchaenterprise_projects_keys_remove_ip_override_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recaptchaenterprise projects keys retrieve legacy secret key.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecaptchaenterpriseV1RetrieveLegacySecretKeyResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn recaptchaenterprise_projects_keys_retrieve_legacy_secret_key(
        &self,
        args: &RecaptchaenterpriseProjectsKeysRetrieveLegacySecretKeyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecaptchaenterpriseV1RetrieveLegacySecretKeyResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recaptchaenterprise_projects_keys_retrieve_legacy_secret_key_builder(
            &self.http_client,
            &args.key,
        )
        .map_err(ProviderError::Api)?;

        let task = recaptchaenterprise_projects_keys_retrieve_legacy_secret_key_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recaptchaenterprise projects relatedaccountgroupmemberships search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecaptchaenterpriseV1SearchRelatedAccountGroupMembershipsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn recaptchaenterprise_projects_relatedaccountgroupmemberships_search(
        &self,
        args: &RecaptchaenterpriseProjectsRelatedaccountgroupmembershipsSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecaptchaenterpriseV1SearchRelatedAccountGroupMembershipsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recaptchaenterprise_projects_relatedaccountgroupmemberships_search_builder(
            &self.http_client,
            &args.project,
        )
        .map_err(ProviderError::Api)?;

        let task = recaptchaenterprise_projects_relatedaccountgroupmemberships_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recaptchaenterprise projects relatedaccountgroups list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecaptchaenterpriseV1ListRelatedAccountGroupsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn recaptchaenterprise_projects_relatedaccountgroups_list(
        &self,
        args: &RecaptchaenterpriseProjectsRelatedaccountgroupsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecaptchaenterpriseV1ListRelatedAccountGroupsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recaptchaenterprise_projects_relatedaccountgroups_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = recaptchaenterprise_projects_relatedaccountgroups_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Recaptchaenterprise projects relatedaccountgroups memberships list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudRecaptchaenterpriseV1ListRelatedAccountGroupMembershipsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn recaptchaenterprise_projects_relatedaccountgroups_memberships_list(
        &self,
        args: &RecaptchaenterpriseProjectsRelatedaccountgroupsMembershipsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudRecaptchaenterpriseV1ListRelatedAccountGroupMembershipsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = recaptchaenterprise_projects_relatedaccountgroups_memberships_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = recaptchaenterprise_projects_relatedaccountgroups_memberships_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
