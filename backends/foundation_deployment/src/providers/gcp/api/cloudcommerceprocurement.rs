//! CloudcommerceprocurementProvider - State-aware cloudcommerceprocurement API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       cloudcommerceprocurement API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::cloudcommerceprocurement::{
    cloudcommerceprocurement_providers_accounts_approve_builder, cloudcommerceprocurement_providers_accounts_approve_task,
    cloudcommerceprocurement_providers_accounts_get_builder, cloudcommerceprocurement_providers_accounts_get_task,
    cloudcommerceprocurement_providers_accounts_list_builder, cloudcommerceprocurement_providers_accounts_list_task,
    cloudcommerceprocurement_providers_accounts_reject_builder, cloudcommerceprocurement_providers_accounts_reject_task,
    cloudcommerceprocurement_providers_accounts_reset_builder, cloudcommerceprocurement_providers_accounts_reset_task,
    cloudcommerceprocurement_providers_entitlements_approve_builder, cloudcommerceprocurement_providers_entitlements_approve_task,
    cloudcommerceprocurement_providers_entitlements_approve_plan_change_builder, cloudcommerceprocurement_providers_entitlements_approve_plan_change_task,
    cloudcommerceprocurement_providers_entitlements_get_builder, cloudcommerceprocurement_providers_entitlements_get_task,
    cloudcommerceprocurement_providers_entitlements_list_builder, cloudcommerceprocurement_providers_entitlements_list_task,
    cloudcommerceprocurement_providers_entitlements_patch_builder, cloudcommerceprocurement_providers_entitlements_patch_task,
    cloudcommerceprocurement_providers_entitlements_reject_builder, cloudcommerceprocurement_providers_entitlements_reject_task,
    cloudcommerceprocurement_providers_entitlements_reject_plan_change_builder, cloudcommerceprocurement_providers_entitlements_reject_plan_change_task,
    cloudcommerceprocurement_providers_entitlements_suspend_builder, cloudcommerceprocurement_providers_entitlements_suspend_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::cloudcommerceprocurement::Account;
use crate::providers::gcp::clients::cloudcommerceprocurement::Empty;
use crate::providers::gcp::clients::cloudcommerceprocurement::Entitlement;
use crate::providers::gcp::clients::cloudcommerceprocurement::ListAccountsResponse;
use crate::providers::gcp::clients::cloudcommerceprocurement::ListEntitlementsResponse;
use crate::providers::gcp::clients::cloudcommerceprocurement::CloudcommerceprocurementProvidersAccountsApproveArgs;
use crate::providers::gcp::clients::cloudcommerceprocurement::CloudcommerceprocurementProvidersAccountsGetArgs;
use crate::providers::gcp::clients::cloudcommerceprocurement::CloudcommerceprocurementProvidersAccountsListArgs;
use crate::providers::gcp::clients::cloudcommerceprocurement::CloudcommerceprocurementProvidersAccountsRejectArgs;
use crate::providers::gcp::clients::cloudcommerceprocurement::CloudcommerceprocurementProvidersAccountsResetArgs;
use crate::providers::gcp::clients::cloudcommerceprocurement::CloudcommerceprocurementProvidersEntitlementsApproveArgs;
use crate::providers::gcp::clients::cloudcommerceprocurement::CloudcommerceprocurementProvidersEntitlementsApprovePlanChangeArgs;
use crate::providers::gcp::clients::cloudcommerceprocurement::CloudcommerceprocurementProvidersEntitlementsGetArgs;
use crate::providers::gcp::clients::cloudcommerceprocurement::CloudcommerceprocurementProvidersEntitlementsListArgs;
use crate::providers::gcp::clients::cloudcommerceprocurement::CloudcommerceprocurementProvidersEntitlementsPatchArgs;
use crate::providers::gcp::clients::cloudcommerceprocurement::CloudcommerceprocurementProvidersEntitlementsRejectArgs;
use crate::providers::gcp::clients::cloudcommerceprocurement::CloudcommerceprocurementProvidersEntitlementsRejectPlanChangeArgs;
use crate::providers::gcp::clients::cloudcommerceprocurement::CloudcommerceprocurementProvidersEntitlementsSuspendArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// CloudcommerceprocurementProvider with automatic state tracking.
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
/// let provider = CloudcommerceprocurementProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct CloudcommerceprocurementProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> CloudcommerceprocurementProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new CloudcommerceprocurementProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Cloudcommerceprocurement providers accounts approve.
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
    pub fn cloudcommerceprocurement_providers_accounts_approve(
        &self,
        args: &CloudcommerceprocurementProvidersAccountsApproveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcommerceprocurement_providers_accounts_approve_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcommerceprocurement_providers_accounts_approve_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudcommerceprocurement providers accounts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Account result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudcommerceprocurement_providers_accounts_get(
        &self,
        args: &CloudcommerceprocurementProvidersAccountsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Account, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcommerceprocurement_providers_accounts_get_builder(
            &self.http_client,
            &args.name,
            &args.view,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcommerceprocurement_providers_accounts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudcommerceprocurement providers accounts list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAccountsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudcommerceprocurement_providers_accounts_list(
        &self,
        args: &CloudcommerceprocurementProvidersAccountsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAccountsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcommerceprocurement_providers_accounts_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcommerceprocurement_providers_accounts_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudcommerceprocurement providers accounts reject.
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
    pub fn cloudcommerceprocurement_providers_accounts_reject(
        &self,
        args: &CloudcommerceprocurementProvidersAccountsRejectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcommerceprocurement_providers_accounts_reject_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcommerceprocurement_providers_accounts_reject_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudcommerceprocurement providers accounts reset.
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
    pub fn cloudcommerceprocurement_providers_accounts_reset(
        &self,
        args: &CloudcommerceprocurementProvidersAccountsResetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcommerceprocurement_providers_accounts_reset_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcommerceprocurement_providers_accounts_reset_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudcommerceprocurement providers entitlements approve.
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
    pub fn cloudcommerceprocurement_providers_entitlements_approve(
        &self,
        args: &CloudcommerceprocurementProvidersEntitlementsApproveArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcommerceprocurement_providers_entitlements_approve_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcommerceprocurement_providers_entitlements_approve_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudcommerceprocurement providers entitlements approve plan change.
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
    pub fn cloudcommerceprocurement_providers_entitlements_approve_plan_change(
        &self,
        args: &CloudcommerceprocurementProvidersEntitlementsApprovePlanChangeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcommerceprocurement_providers_entitlements_approve_plan_change_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcommerceprocurement_providers_entitlements_approve_plan_change_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudcommerceprocurement providers entitlements get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Entitlement result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudcommerceprocurement_providers_entitlements_get(
        &self,
        args: &CloudcommerceprocurementProvidersEntitlementsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Entitlement, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcommerceprocurement_providers_entitlements_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcommerceprocurement_providers_entitlements_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudcommerceprocurement providers entitlements list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListEntitlementsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn cloudcommerceprocurement_providers_entitlements_list(
        &self,
        args: &CloudcommerceprocurementProvidersEntitlementsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListEntitlementsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcommerceprocurement_providers_entitlements_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcommerceprocurement_providers_entitlements_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudcommerceprocurement providers entitlements patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Entitlement result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn cloudcommerceprocurement_providers_entitlements_patch(
        &self,
        args: &CloudcommerceprocurementProvidersEntitlementsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Entitlement, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcommerceprocurement_providers_entitlements_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcommerceprocurement_providers_entitlements_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudcommerceprocurement providers entitlements reject.
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
    pub fn cloudcommerceprocurement_providers_entitlements_reject(
        &self,
        args: &CloudcommerceprocurementProvidersEntitlementsRejectArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcommerceprocurement_providers_entitlements_reject_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcommerceprocurement_providers_entitlements_reject_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudcommerceprocurement providers entitlements reject plan change.
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
    pub fn cloudcommerceprocurement_providers_entitlements_reject_plan_change(
        &self,
        args: &CloudcommerceprocurementProvidersEntitlementsRejectPlanChangeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcommerceprocurement_providers_entitlements_reject_plan_change_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcommerceprocurement_providers_entitlements_reject_plan_change_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Cloudcommerceprocurement providers entitlements suspend.
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
    pub fn cloudcommerceprocurement_providers_entitlements_suspend(
        &self,
        args: &CloudcommerceprocurementProvidersEntitlementsSuspendArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = cloudcommerceprocurement_providers_entitlements_suspend_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = cloudcommerceprocurement_providers_entitlements_suspend_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
