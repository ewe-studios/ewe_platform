//! BillingbudgetsProvider - State-aware billingbudgets API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       billingbudgets API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::billingbudgets::{
    billingbudgets_billing_accounts_budgets_create_builder, billingbudgets_billing_accounts_budgets_create_task,
    billingbudgets_billing_accounts_budgets_delete_builder, billingbudgets_billing_accounts_budgets_delete_task,
    billingbudgets_billing_accounts_budgets_get_builder, billingbudgets_billing_accounts_budgets_get_task,
    billingbudgets_billing_accounts_budgets_list_builder, billingbudgets_billing_accounts_budgets_list_task,
    billingbudgets_billing_accounts_budgets_patch_builder, billingbudgets_billing_accounts_budgets_patch_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::billingbudgets::GoogleCloudBillingBudgetsV1Budget;
use crate::providers::gcp::clients::billingbudgets::GoogleCloudBillingBudgetsV1ListBudgetsResponse;
use crate::providers::gcp::clients::billingbudgets::GoogleProtobufEmpty;
use crate::providers::gcp::clients::billingbudgets::BillingbudgetsBillingAccountsBudgetsCreateArgs;
use crate::providers::gcp::clients::billingbudgets::BillingbudgetsBillingAccountsBudgetsDeleteArgs;
use crate::providers::gcp::clients::billingbudgets::BillingbudgetsBillingAccountsBudgetsGetArgs;
use crate::providers::gcp::clients::billingbudgets::BillingbudgetsBillingAccountsBudgetsListArgs;
use crate::providers::gcp::clients::billingbudgets::BillingbudgetsBillingAccountsBudgetsPatchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// BillingbudgetsProvider with automatic state tracking.
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
/// let provider = BillingbudgetsProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct BillingbudgetsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> BillingbudgetsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new BillingbudgetsProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Billingbudgets billing accounts budgets create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudBillingBudgetsV1Budget result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn billingbudgets_billing_accounts_budgets_create(
        &self,
        args: &BillingbudgetsBillingAccountsBudgetsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudBillingBudgetsV1Budget, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = billingbudgets_billing_accounts_budgets_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = billingbudgets_billing_accounts_budgets_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Billingbudgets billing accounts budgets delete.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn billingbudgets_billing_accounts_budgets_delete(
        &self,
        args: &BillingbudgetsBillingAccountsBudgetsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = billingbudgets_billing_accounts_budgets_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = billingbudgets_billing_accounts_budgets_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Billingbudgets billing accounts budgets get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudBillingBudgetsV1Budget result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn billingbudgets_billing_accounts_budgets_get(
        &self,
        args: &BillingbudgetsBillingAccountsBudgetsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudBillingBudgetsV1Budget, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = billingbudgets_billing_accounts_budgets_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = billingbudgets_billing_accounts_budgets_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Billingbudgets billing accounts budgets list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudBillingBudgetsV1ListBudgetsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn billingbudgets_billing_accounts_budgets_list(
        &self,
        args: &BillingbudgetsBillingAccountsBudgetsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudBillingBudgetsV1ListBudgetsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = billingbudgets_billing_accounts_budgets_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
            &args.scope,
        )
        .map_err(ProviderError::Api)?;

        let task = billingbudgets_billing_accounts_budgets_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Billingbudgets billing accounts budgets patch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudBillingBudgetsV1Budget result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn billingbudgets_billing_accounts_budgets_patch(
        &self,
        args: &BillingbudgetsBillingAccountsBudgetsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudBillingBudgetsV1Budget, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = billingbudgets_billing_accounts_budgets_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = billingbudgets_billing_accounts_budgets_patch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
