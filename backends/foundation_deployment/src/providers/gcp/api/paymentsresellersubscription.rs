//! PaymentsresellersubscriptionProvider - State-aware paymentsresellersubscription API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       paymentsresellersubscription API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::paymentsresellersubscription::{
    paymentsresellersubscription_partners_products_list_builder, paymentsresellersubscription_partners_products_list_task,
    paymentsresellersubscription_partners_promotions_find_eligible_builder, paymentsresellersubscription_partners_promotions_find_eligible_task,
    paymentsresellersubscription_partners_promotions_list_builder, paymentsresellersubscription_partners_promotions_list_task,
    paymentsresellersubscription_partners_subscriptions_cancel_builder, paymentsresellersubscription_partners_subscriptions_cancel_task,
    paymentsresellersubscription_partners_subscriptions_create_builder, paymentsresellersubscription_partners_subscriptions_create_task,
    paymentsresellersubscription_partners_subscriptions_entitle_builder, paymentsresellersubscription_partners_subscriptions_entitle_task,
    paymentsresellersubscription_partners_subscriptions_extend_builder, paymentsresellersubscription_partners_subscriptions_extend_task,
    paymentsresellersubscription_partners_subscriptions_get_builder, paymentsresellersubscription_partners_subscriptions_get_task,
    paymentsresellersubscription_partners_subscriptions_provision_builder, paymentsresellersubscription_partners_subscriptions_provision_task,
    paymentsresellersubscription_partners_subscriptions_resume_builder, paymentsresellersubscription_partners_subscriptions_resume_task,
    paymentsresellersubscription_partners_subscriptions_suspend_builder, paymentsresellersubscription_partners_subscriptions_suspend_task,
    paymentsresellersubscription_partners_subscriptions_undo_cancel_builder, paymentsresellersubscription_partners_subscriptions_undo_cancel_task,
    paymentsresellersubscription_partners_subscriptions_line_items_patch_builder, paymentsresellersubscription_partners_subscriptions_line_items_patch_task,
    paymentsresellersubscription_partners_user_sessions_generate_builder, paymentsresellersubscription_partners_user_sessions_generate_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::paymentsresellersubscription::CancelSubscriptionResponse;
use crate::providers::gcp::clients::paymentsresellersubscription::EntitleSubscriptionResponse;
use crate::providers::gcp::clients::paymentsresellersubscription::ExtendSubscriptionResponse;
use crate::providers::gcp::clients::paymentsresellersubscription::FindEligiblePromotionsResponse;
use crate::providers::gcp::clients::paymentsresellersubscription::GenerateUserSessionResponse;
use crate::providers::gcp::clients::paymentsresellersubscription::ListProductsResponse;
use crate::providers::gcp::clients::paymentsresellersubscription::ListPromotionsResponse;
use crate::providers::gcp::clients::paymentsresellersubscription::ResumeSubscriptionResponse;
use crate::providers::gcp::clients::paymentsresellersubscription::Subscription;
use crate::providers::gcp::clients::paymentsresellersubscription::SubscriptionLineItem;
use crate::providers::gcp::clients::paymentsresellersubscription::SuspendSubscriptionResponse;
use crate::providers::gcp::clients::paymentsresellersubscription::UndoCancelSubscriptionResponse;
use crate::providers::gcp::clients::paymentsresellersubscription::PaymentsresellersubscriptionPartnersProductsListArgs;
use crate::providers::gcp::clients::paymentsresellersubscription::PaymentsresellersubscriptionPartnersPromotionsFindEligibleArgs;
use crate::providers::gcp::clients::paymentsresellersubscription::PaymentsresellersubscriptionPartnersPromotionsListArgs;
use crate::providers::gcp::clients::paymentsresellersubscription::PaymentsresellersubscriptionPartnersSubscriptionsCancelArgs;
use crate::providers::gcp::clients::paymentsresellersubscription::PaymentsresellersubscriptionPartnersSubscriptionsCreateArgs;
use crate::providers::gcp::clients::paymentsresellersubscription::PaymentsresellersubscriptionPartnersSubscriptionsEntitleArgs;
use crate::providers::gcp::clients::paymentsresellersubscription::PaymentsresellersubscriptionPartnersSubscriptionsExtendArgs;
use crate::providers::gcp::clients::paymentsresellersubscription::PaymentsresellersubscriptionPartnersSubscriptionsGetArgs;
use crate::providers::gcp::clients::paymentsresellersubscription::PaymentsresellersubscriptionPartnersSubscriptionsLineItemsPatchArgs;
use crate::providers::gcp::clients::paymentsresellersubscription::PaymentsresellersubscriptionPartnersSubscriptionsProvisionArgs;
use crate::providers::gcp::clients::paymentsresellersubscription::PaymentsresellersubscriptionPartnersSubscriptionsResumeArgs;
use crate::providers::gcp::clients::paymentsresellersubscription::PaymentsresellersubscriptionPartnersSubscriptionsSuspendArgs;
use crate::providers::gcp::clients::paymentsresellersubscription::PaymentsresellersubscriptionPartnersSubscriptionsUndoCancelArgs;
use crate::providers::gcp::clients::paymentsresellersubscription::PaymentsresellersubscriptionPartnersUserSessionsGenerateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// PaymentsresellersubscriptionProvider with automatic state tracking.
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
/// let provider = PaymentsresellersubscriptionProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct PaymentsresellersubscriptionProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> PaymentsresellersubscriptionProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new PaymentsresellersubscriptionProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Paymentsresellersubscription partners products list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListProductsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn paymentsresellersubscription_partners_products_list(
        &self,
        args: &PaymentsresellersubscriptionPartnersProductsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListProductsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = paymentsresellersubscription_partners_products_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = paymentsresellersubscription_partners_products_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Paymentsresellersubscription partners promotions find eligible.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FindEligiblePromotionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn paymentsresellersubscription_partners_promotions_find_eligible(
        &self,
        args: &PaymentsresellersubscriptionPartnersPromotionsFindEligibleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FindEligiblePromotionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = paymentsresellersubscription_partners_promotions_find_eligible_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = paymentsresellersubscription_partners_promotions_find_eligible_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Paymentsresellersubscription partners promotions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListPromotionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn paymentsresellersubscription_partners_promotions_list(
        &self,
        args: &PaymentsresellersubscriptionPartnersPromotionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListPromotionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = paymentsresellersubscription_partners_promotions_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = paymentsresellersubscription_partners_promotions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Paymentsresellersubscription partners subscriptions cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CancelSubscriptionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn paymentsresellersubscription_partners_subscriptions_cancel(
        &self,
        args: &PaymentsresellersubscriptionPartnersSubscriptionsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CancelSubscriptionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = paymentsresellersubscription_partners_subscriptions_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = paymentsresellersubscription_partners_subscriptions_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Paymentsresellersubscription partners subscriptions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subscription result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn paymentsresellersubscription_partners_subscriptions_create(
        &self,
        args: &PaymentsresellersubscriptionPartnersSubscriptionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = paymentsresellersubscription_partners_subscriptions_create_builder(
            &self.http_client,
            &args.parent,
            &args.subscriptionId,
        )
        .map_err(ProviderError::Api)?;

        let task = paymentsresellersubscription_partners_subscriptions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Paymentsresellersubscription partners subscriptions entitle.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the EntitleSubscriptionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn paymentsresellersubscription_partners_subscriptions_entitle(
        &self,
        args: &PaymentsresellersubscriptionPartnersSubscriptionsEntitleArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<EntitleSubscriptionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = paymentsresellersubscription_partners_subscriptions_entitle_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = paymentsresellersubscription_partners_subscriptions_entitle_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Paymentsresellersubscription partners subscriptions extend.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ExtendSubscriptionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn paymentsresellersubscription_partners_subscriptions_extend(
        &self,
        args: &PaymentsresellersubscriptionPartnersSubscriptionsExtendArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ExtendSubscriptionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = paymentsresellersubscription_partners_subscriptions_extend_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = paymentsresellersubscription_partners_subscriptions_extend_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Paymentsresellersubscription partners subscriptions get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subscription result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn paymentsresellersubscription_partners_subscriptions_get(
        &self,
        args: &PaymentsresellersubscriptionPartnersSubscriptionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = paymentsresellersubscription_partners_subscriptions_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = paymentsresellersubscription_partners_subscriptions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Paymentsresellersubscription partners subscriptions provision.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subscription result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn paymentsresellersubscription_partners_subscriptions_provision(
        &self,
        args: &PaymentsresellersubscriptionPartnersSubscriptionsProvisionArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = paymentsresellersubscription_partners_subscriptions_provision_builder(
            &self.http_client,
            &args.parent,
            &args.cycleOptions.initialCycleDuration.count,
            &args.cycleOptions.initialCycleDuration.unit,
            &args.subscriptionId,
        )
        .map_err(ProviderError::Api)?;

        let task = paymentsresellersubscription_partners_subscriptions_provision_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Paymentsresellersubscription partners subscriptions resume.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ResumeSubscriptionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn paymentsresellersubscription_partners_subscriptions_resume(
        &self,
        args: &PaymentsresellersubscriptionPartnersSubscriptionsResumeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ResumeSubscriptionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = paymentsresellersubscription_partners_subscriptions_resume_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = paymentsresellersubscription_partners_subscriptions_resume_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Paymentsresellersubscription partners subscriptions suspend.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SuspendSubscriptionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn paymentsresellersubscription_partners_subscriptions_suspend(
        &self,
        args: &PaymentsresellersubscriptionPartnersSubscriptionsSuspendArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SuspendSubscriptionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = paymentsresellersubscription_partners_subscriptions_suspend_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = paymentsresellersubscription_partners_subscriptions_suspend_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Paymentsresellersubscription partners subscriptions undo cancel.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UndoCancelSubscriptionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn paymentsresellersubscription_partners_subscriptions_undo_cancel(
        &self,
        args: &PaymentsresellersubscriptionPartnersSubscriptionsUndoCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UndoCancelSubscriptionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = paymentsresellersubscription_partners_subscriptions_undo_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = paymentsresellersubscription_partners_subscriptions_undo_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Paymentsresellersubscription partners subscriptions line items patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SubscriptionLineItem result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn paymentsresellersubscription_partners_subscriptions_line_items_patch(
        &self,
        args: &PaymentsresellersubscriptionPartnersSubscriptionsLineItemsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SubscriptionLineItem, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = paymentsresellersubscription_partners_subscriptions_line_items_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = paymentsresellersubscription_partners_subscriptions_line_items_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Paymentsresellersubscription partners user sessions generate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenerateUserSessionResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn paymentsresellersubscription_partners_user_sessions_generate(
        &self,
        args: &PaymentsresellersubscriptionPartnersUserSessionsGenerateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenerateUserSessionResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = paymentsresellersubscription_partners_user_sessions_generate_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = paymentsresellersubscription_partners_user_sessions_generate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
