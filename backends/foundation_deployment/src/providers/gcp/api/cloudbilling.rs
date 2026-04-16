//! CloudbillingProvider - State-aware cloudbilling API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       cloudbilling API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::cloudbilling::{
    cloudbilling_billing_accounts_create_builder, cloudbilling_billing_accounts_create_task,
    cloudbilling_billing_accounts_get_builder, cloudbilling_billing_accounts_get_task,
    cloudbilling_billing_accounts_get_iam_policy_builder, cloudbilling_billing_accounts_get_iam_policy_task,
    cloudbilling_billing_accounts_list_builder, cloudbilling_billing_accounts_list_task,
    cloudbilling_billing_accounts_move_builder, cloudbilling_billing_accounts_move_task,
    cloudbilling_billing_accounts_patch_builder, cloudbilling_billing_accounts_patch_task,
    cloudbilling_billing_accounts_set_iam_policy_builder, cloudbilling_billing_accounts_set_iam_policy_task,
    cloudbilling_billing_accounts_test_iam_permissions_builder, cloudbilling_billing_accounts_test_iam_permissions_task,
    cloudbilling_billing_accounts_projects_list_builder, cloudbilling_billing_accounts_projects_list_task,
    cloudbilling_billing_accounts_sub_accounts_create_builder, cloudbilling_billing_accounts_sub_accounts_create_task,
    cloudbilling_billing_accounts_sub_accounts_list_builder, cloudbilling_billing_accounts_sub_accounts_list_task,
    cloudbilling_organizations_billing_accounts_create_builder, cloudbilling_organizations_billing_accounts_create_task,
    cloudbilling_organizations_billing_accounts_list_builder, cloudbilling_organizations_billing_accounts_list_task,
    cloudbilling_organizations_billing_accounts_move_builder, cloudbilling_organizations_billing_accounts_move_task,
    cloudbilling_projects_get_billing_info_builder, cloudbilling_projects_get_billing_info_task,
    cloudbilling_projects_update_billing_info_builder, cloudbilling_projects_update_billing_info_task,
    cloudbilling_services_list_builder, cloudbilling_services_list_task,
    cloudbilling_services_skus_list_builder, cloudbilling_services_skus_list_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::cloudbilling::BillingAccount;
use crate::providers::gcp::clients::cloudbilling::ListBillingAccountsResponse;
use crate::providers::gcp::clients::cloudbilling::ListProjectBillingInfoResponse;
use crate::providers::gcp::clients::cloudbilling::ListServicesResponse;
use crate::providers::gcp::clients::cloudbilling::ListSkusResponse;
use crate::providers::gcp::clients::cloudbilling::Policy;
use crate::providers::gcp::clients::cloudbilling::ProjectBillingInfo;
use crate::providers::gcp::clients::cloudbilling::TestIamPermissionsResponse;
use crate::providers::gcp::clients::cloudbilling::CloudbillingBillingAccountsCreateArgs;
use crate::providers::gcp::clients::cloudbilling::CloudbillingBillingAccountsGetArgs;
use crate::providers::gcp::clients::cloudbilling::CloudbillingBillingAccountsGetIamPolicyArgs;
use crate::providers::gcp::clients::cloudbilling::CloudbillingBillingAccountsListArgs;
use crate::providers::gcp::clients::cloudbilling::CloudbillingBillingAccountsMoveArgs;
use crate::providers::gcp::clients::cloudbilling::CloudbillingBillingAccountsPatchArgs;
use crate::providers::gcp::clients::cloudbilling::CloudbillingBillingAccountsProjectsListArgs;
use crate::providers::gcp::clients::cloudbilling::CloudbillingBillingAccountsSetIamPolicyArgs;
use crate::providers::gcp::clients::cloudbilling::CloudbillingBillingAccountsSubAccountsCreateArgs;
use crate::providers::gcp::clients::cloudbilling::CloudbillingBillingAccountsSubAccountsListArgs;
use crate::providers::gcp::clients::cloudbilling::CloudbillingBillingAccountsTestIamPermissionsArgs;
use crate::providers::gcp::clients::cloudbilling::CloudbillingOrganizationsBillingAccountsCreateArgs;
use crate::providers::gcp::clients::cloudbilling::CloudbillingOrganizationsBillingAccountsListArgs;
use crate::providers::gcp::clients::cloudbilling::CloudbillingOrganizationsBillingAccountsMoveArgs;
use crate::providers::gcp::clients::cloudbilling::CloudbillingProjectsGetBillingInfoArgs;
use crate::providers::gcp::clients::cloudbilling::CloudbillingProjectsUpdateBillingInfoArgs;
use crate::providers::gcp::clients::cloudbilling::CloudbillingServicesListArgs;
use crate::providers::gcp::clients::cloudbilling::CloudbillingServicesSkusListArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// CloudbillingProvider with automatic state tracking.
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
/// let provider = CloudbillingProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct CloudbillingProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> CloudbillingProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new CloudbillingProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new CloudbillingProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Cloudbilling billing accounts create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudbilling_billing_accounts_create(
        &self,
        args: &CloudbillingBillingAccountsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingAccount, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbilling_billing_accounts_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbilling_billing_accounts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbilling billing accounts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudbilling_billing_accounts_get(
        &self,
        args: &CloudbillingBillingAccountsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingAccount, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbilling_billing_accounts_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbilling_billing_accounts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbilling billing accounts get iam policy.
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
    pub fn cloudbilling_billing_accounts_get_iam_policy(
        &self,
        args: &CloudbillingBillingAccountsGetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbilling_billing_accounts_get_iam_policy_builder(
            &self.http_client,
            &args.resource,
            &args.options_requestedPolicyVersion,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbilling_billing_accounts_get_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbilling billing accounts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBillingAccountsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudbilling_billing_accounts_list(
        &self,
        args: &CloudbillingBillingAccountsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBillingAccountsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbilling_billing_accounts_list_builder(
            &self.http_client,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbilling_billing_accounts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbilling billing accounts move.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudbilling_billing_accounts_move(
        &self,
        args: &CloudbillingBillingAccountsMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingAccount, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbilling_billing_accounts_move_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbilling_billing_accounts_move_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbilling billing accounts patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudbilling_billing_accounts_patch(
        &self,
        args: &CloudbillingBillingAccountsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingAccount, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbilling_billing_accounts_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbilling_billing_accounts_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbilling billing accounts set iam policy.
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
    pub fn cloudbilling_billing_accounts_set_iam_policy(
        &self,
        args: &CloudbillingBillingAccountsSetIamPolicyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Policy, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbilling_billing_accounts_set_iam_policy_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbilling_billing_accounts_set_iam_policy_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbilling billing accounts test iam permissions.
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
    pub fn cloudbilling_billing_accounts_test_iam_permissions(
        &self,
        args: &CloudbillingBillingAccountsTestIamPermissionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TestIamPermissionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbilling_billing_accounts_test_iam_permissions_builder(
            &self.http_client,
            &args.resource,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbilling_billing_accounts_test_iam_permissions_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbilling billing accounts projects list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListProjectBillingInfoResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudbilling_billing_accounts_projects_list(
        &self,
        args: &CloudbillingBillingAccountsProjectsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListProjectBillingInfoResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbilling_billing_accounts_projects_list_builder(
            &self.http_client,
            &args.name,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbilling_billing_accounts_projects_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbilling billing accounts sub accounts create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudbilling_billing_accounts_sub_accounts_create(
        &self,
        args: &CloudbillingBillingAccountsSubAccountsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingAccount, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbilling_billing_accounts_sub_accounts_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbilling_billing_accounts_sub_accounts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbilling billing accounts sub accounts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBillingAccountsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudbilling_billing_accounts_sub_accounts_list(
        &self,
        args: &CloudbillingBillingAccountsSubAccountsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBillingAccountsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbilling_billing_accounts_sub_accounts_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbilling_billing_accounts_sub_accounts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbilling organizations billing accounts create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudbilling_organizations_billing_accounts_create(
        &self,
        args: &CloudbillingOrganizationsBillingAccountsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingAccount, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbilling_organizations_billing_accounts_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbilling_organizations_billing_accounts_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbilling organizations billing accounts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListBillingAccountsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudbilling_organizations_billing_accounts_list(
        &self,
        args: &CloudbillingOrganizationsBillingAccountsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListBillingAccountsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbilling_organizations_billing_accounts_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbilling_organizations_billing_accounts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbilling organizations billing accounts move.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the BillingAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudbilling_organizations_billing_accounts_move(
        &self,
        args: &CloudbillingOrganizationsBillingAccountsMoveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<BillingAccount, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbilling_organizations_billing_accounts_move_builder(
            &self.http_client,
            &args.destinationParent,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbilling_organizations_billing_accounts_move_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbilling projects get billing info.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectBillingInfo result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudbilling_projects_get_billing_info(
        &self,
        args: &CloudbillingProjectsGetBillingInfoArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectBillingInfo, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbilling_projects_get_billing_info_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbilling_projects_get_billing_info_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbilling projects update billing info.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProjectBillingInfo result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudbilling_projects_update_billing_info(
        &self,
        args: &CloudbillingProjectsUpdateBillingInfoArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProjectBillingInfo, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbilling_projects_update_billing_info_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbilling_projects_update_billing_info_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbilling services list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListServicesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudbilling_services_list(
        &self,
        args: &CloudbillingServicesListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListServicesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbilling_services_list_builder(
            &self.http_client,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbilling_services_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudbilling services skus list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListSkusResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudbilling_services_skus_list(
        &self,
        args: &CloudbillingServicesSkusListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListSkusResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudbilling_services_skus_list_builder(
            &self.http_client,
            &args.parent,
            &args.currencyCode,
            &args.endTime,
            &args.pageSize,
            &args.pageToken,
            &args.startTime,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudbilling_services_skus_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
