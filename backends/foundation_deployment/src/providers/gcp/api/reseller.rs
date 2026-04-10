//! ResellerProvider - State-aware reseller API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       reseller API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::reseller::{
    reseller_customers_insert_builder, reseller_customers_insert_task,
    reseller_customers_patch_builder, reseller_customers_patch_task,
    reseller_customers_update_builder, reseller_customers_update_task,
    reseller_resellernotify_register_builder, reseller_resellernotify_register_task,
    reseller_resellernotify_unregister_builder, reseller_resellernotify_unregister_task,
    reseller_subscriptions_activate_builder, reseller_subscriptions_activate_task,
    reseller_subscriptions_change_plan_builder, reseller_subscriptions_change_plan_task,
    reseller_subscriptions_change_renewal_settings_builder, reseller_subscriptions_change_renewal_settings_task,
    reseller_subscriptions_change_seats_builder, reseller_subscriptions_change_seats_task,
    reseller_subscriptions_delete_builder, reseller_subscriptions_delete_task,
    reseller_subscriptions_insert_builder, reseller_subscriptions_insert_task,
    reseller_subscriptions_start_paid_service_builder, reseller_subscriptions_start_paid_service_task,
    reseller_subscriptions_suspend_builder, reseller_subscriptions_suspend_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::reseller::Customer;
use crate::providers::gcp::clients::reseller::ResellernotifyResource;
use crate::providers::gcp::clients::reseller::Subscription;
use crate::providers::gcp::clients::reseller::ResellerCustomersInsertArgs;
use crate::providers::gcp::clients::reseller::ResellerCustomersPatchArgs;
use crate::providers::gcp::clients::reseller::ResellerCustomersUpdateArgs;
use crate::providers::gcp::clients::reseller::ResellerResellernotifyRegisterArgs;
use crate::providers::gcp::clients::reseller::ResellerResellernotifyUnregisterArgs;
use crate::providers::gcp::clients::reseller::ResellerSubscriptionsActivateArgs;
use crate::providers::gcp::clients::reseller::ResellerSubscriptionsChangePlanArgs;
use crate::providers::gcp::clients::reseller::ResellerSubscriptionsChangeRenewalSettingsArgs;
use crate::providers::gcp::clients::reseller::ResellerSubscriptionsChangeSeatsArgs;
use crate::providers::gcp::clients::reseller::ResellerSubscriptionsDeleteArgs;
use crate::providers::gcp::clients::reseller::ResellerSubscriptionsInsertArgs;
use crate::providers::gcp::clients::reseller::ResellerSubscriptionsStartPaidServiceArgs;
use crate::providers::gcp::clients::reseller::ResellerSubscriptionsSuspendArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ResellerProvider with automatic state tracking.
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
/// let provider = ResellerProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct ResellerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> ResellerProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new ResellerProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Reseller customers insert.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Customer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn reseller_customers_insert(
        &self,
        args: &ResellerCustomersInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Customer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reseller_customers_insert_builder(
            &self.http_client,
            &args.customerAuthToken,
        )
        .map_err(ProviderError::Api)?;

        let task = reseller_customers_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Reseller customers patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Customer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn reseller_customers_patch(
        &self,
        args: &ResellerCustomersPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Customer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reseller_customers_patch_builder(
            &self.http_client,
            &args.customerId,
        )
        .map_err(ProviderError::Api)?;

        let task = reseller_customers_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Reseller customers update.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Customer result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn reseller_customers_update(
        &self,
        args: &ResellerCustomersUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Customer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reseller_customers_update_builder(
            &self.http_client,
            &args.customerId,
        )
        .map_err(ProviderError::Api)?;

        let task = reseller_customers_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Reseller resellernotify register.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ResellernotifyResource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn reseller_resellernotify_register(
        &self,
        args: &ResellerResellernotifyRegisterArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ResellernotifyResource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reseller_resellernotify_register_builder(
            &self.http_client,
            &args.serviceAccountEmailAddress,
        )
        .map_err(ProviderError::Api)?;

        let task = reseller_resellernotify_register_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Reseller resellernotify unregister.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ResellernotifyResource result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn reseller_resellernotify_unregister(
        &self,
        args: &ResellerResellernotifyUnregisterArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ResellernotifyResource, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reseller_resellernotify_unregister_builder(
            &self.http_client,
            &args.serviceAccountEmailAddress,
        )
        .map_err(ProviderError::Api)?;

        let task = reseller_resellernotify_unregister_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Reseller subscriptions activate.
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
    pub fn reseller_subscriptions_activate(
        &self,
        args: &ResellerSubscriptionsActivateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reseller_subscriptions_activate_builder(
            &self.http_client,
            &args.customerId,
            &args.subscriptionId,
        )
        .map_err(ProviderError::Api)?;

        let task = reseller_subscriptions_activate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Reseller subscriptions change plan.
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
    pub fn reseller_subscriptions_change_plan(
        &self,
        args: &ResellerSubscriptionsChangePlanArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reseller_subscriptions_change_plan_builder(
            &self.http_client,
            &args.customerId,
            &args.subscriptionId,
        )
        .map_err(ProviderError::Api)?;

        let task = reseller_subscriptions_change_plan_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Reseller subscriptions change renewal settings.
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
    pub fn reseller_subscriptions_change_renewal_settings(
        &self,
        args: &ResellerSubscriptionsChangeRenewalSettingsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reseller_subscriptions_change_renewal_settings_builder(
            &self.http_client,
            &args.customerId,
            &args.subscriptionId,
        )
        .map_err(ProviderError::Api)?;

        let task = reseller_subscriptions_change_renewal_settings_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Reseller subscriptions change seats.
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
    pub fn reseller_subscriptions_change_seats(
        &self,
        args: &ResellerSubscriptionsChangeSeatsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reseller_subscriptions_change_seats_builder(
            &self.http_client,
            &args.customerId,
            &args.subscriptionId,
        )
        .map_err(ProviderError::Api)?;

        let task = reseller_subscriptions_change_seats_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Reseller subscriptions delete.
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
    pub fn reseller_subscriptions_delete(
        &self,
        args: &ResellerSubscriptionsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<serde_json::Value, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reseller_subscriptions_delete_builder(
            &self.http_client,
            &args.customerId,
            &args.subscriptionId,
            &args.deletionType,
            &args.deletionType,
        )
        .map_err(ProviderError::Api)?;

        let task = reseller_subscriptions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Reseller subscriptions insert.
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
    pub fn reseller_subscriptions_insert(
        &self,
        args: &ResellerSubscriptionsInsertArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reseller_subscriptions_insert_builder(
            &self.http_client,
            &args.customerId,
            &args.action,
            &args.customerAuthToken,
            &args.sourceSkuId,
        )
        .map_err(ProviderError::Api)?;

        let task = reseller_subscriptions_insert_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Reseller subscriptions start paid service.
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
    pub fn reseller_subscriptions_start_paid_service(
        &self,
        args: &ResellerSubscriptionsStartPaidServiceArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reseller_subscriptions_start_paid_service_builder(
            &self.http_client,
            &args.customerId,
            &args.subscriptionId,
        )
        .map_err(ProviderError::Api)?;

        let task = reseller_subscriptions_start_paid_service_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Reseller subscriptions suspend.
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
    pub fn reseller_subscriptions_suspend(
        &self,
        args: &ResellerSubscriptionsSuspendArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reseller_subscriptions_suspend_builder(
            &self.http_client,
            &args.customerId,
            &args.subscriptionId,
        )
        .map_err(ProviderError::Api)?;

        let task = reseller_subscriptions_suspend_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
