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
    reseller_customers_get_builder, reseller_customers_get_task,
    reseller_customers_insert_builder, reseller_customers_insert_task,
    reseller_customers_patch_builder, reseller_customers_patch_task,
    reseller_customers_update_builder, reseller_customers_update_task,
    reseller_resellernotify_getwatchdetails_builder, reseller_resellernotify_getwatchdetails_task,
    reseller_resellernotify_register_builder, reseller_resellernotify_register_task,
    reseller_resellernotify_unregister_builder, reseller_resellernotify_unregister_task,
    reseller_subscriptions_activate_builder, reseller_subscriptions_activate_task,
    reseller_subscriptions_change_plan_builder, reseller_subscriptions_change_plan_task,
    reseller_subscriptions_change_renewal_settings_builder, reseller_subscriptions_change_renewal_settings_task,
    reseller_subscriptions_change_seats_builder, reseller_subscriptions_change_seats_task,
    reseller_subscriptions_delete_builder, reseller_subscriptions_delete_task,
    reseller_subscriptions_get_builder, reseller_subscriptions_get_task,
    reseller_subscriptions_insert_builder, reseller_subscriptions_insert_task,
    reseller_subscriptions_list_builder, reseller_subscriptions_list_task,
    reseller_subscriptions_start_paid_service_builder, reseller_subscriptions_start_paid_service_task,
    reseller_subscriptions_suspend_builder, reseller_subscriptions_suspend_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::reseller::Customer;
use crate::providers::gcp::clients::reseller::ResellernotifyGetwatchdetailsResponse;
use crate::providers::gcp::clients::reseller::ResellernotifyResource;
use crate::providers::gcp::clients::reseller::Subscription;
use crate::providers::gcp::clients::reseller::Subscriptions;
use crate::providers::gcp::clients::reseller::ResellerCustomersGetArgs;
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
use crate::providers::gcp::clients::reseller::ResellerSubscriptionsGetArgs;
use crate::providers::gcp::clients::reseller::ResellerSubscriptionsInsertArgs;
use crate::providers::gcp::clients::reseller::ResellerSubscriptionsListArgs;
use crate::providers::gcp::clients::reseller::ResellerSubscriptionsStartPaidServiceArgs;
use crate::providers::gcp::clients::reseller::ResellerSubscriptionsSuspendArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ResellerProvider with automatic state tracking.
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
/// let provider = ResellerProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct ResellerProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> ResellerProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new ResellerProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new ResellerProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Reseller customers get.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn reseller_customers_get(
        &self,
        args: &ResellerCustomersGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Customer, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reseller_customers_get_builder(
            &self.http_client,
            &args.customerId,
        )
        .map_err(ProviderError::Api)?;

        let task = reseller_customers_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Reseller resellernotify getwatchdetails.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ResellernotifyGetwatchdetailsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn reseller_resellernotify_getwatchdetails(
        &self,
        args: &ResellerResellernotifyGetwatchdetailsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ResellernotifyGetwatchdetailsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reseller_resellernotify_getwatchdetails_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = reseller_resellernotify_getwatchdetails_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
        )
        .map_err(ProviderError::Api)?;

        let task = reseller_subscriptions_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Reseller subscriptions get.
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
    pub fn reseller_subscriptions_get(
        &self,
        args: &ResellerSubscriptionsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscription, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reseller_subscriptions_get_builder(
            &self.http_client,
            &args.customerId,
            &args.subscriptionId,
        )
        .map_err(ProviderError::Api)?;

        let task = reseller_subscriptions_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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

    /// Reseller subscriptions list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Subscriptions result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn reseller_subscriptions_list(
        &self,
        args: &ResellerSubscriptionsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Subscriptions, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = reseller_subscriptions_list_builder(
            &self.http_client,
            &args.customerAuthToken,
            &args.customerId,
            &args.customerNamePrefix,
            &args.maxResults,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = reseller_subscriptions_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
