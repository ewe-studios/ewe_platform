//! AppsmarketProvider - State-aware appsmarket API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       appsmarket API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::appsmarket::{
    appsmarket_customer_license_get_builder, appsmarket_customer_license_get_task,
    appsmarket_user_license_get_builder, appsmarket_user_license_get_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::appsmarket::CustomerLicense;
use crate::providers::gcp::clients::appsmarket::UserLicense;
use crate::providers::gcp::clients::appsmarket::AppsmarketCustomerLicenseGetArgs;
use crate::providers::gcp::clients::appsmarket::AppsmarketUserLicenseGetArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AppsmarketProvider with automatic state tracking.
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
/// let provider = AppsmarketProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct AppsmarketProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> AppsmarketProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new AppsmarketProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new AppsmarketProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Appsmarket customer license get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CustomerLicense result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn appsmarket_customer_license_get(
        &self,
        args: &AppsmarketCustomerLicenseGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CustomerLicense, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appsmarket_customer_license_get_builder(
            &self.http_client,
            &args.applicationId,
            &args.customerId,
        )
        .map_err(ProviderError::Api)?;

        let task = appsmarket_customer_license_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Appsmarket user license get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the UserLicense result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn appsmarket_user_license_get(
        &self,
        args: &AppsmarketUserLicenseGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<UserLicense, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = appsmarket_user_license_get_builder(
            &self.http_client,
            &args.applicationId,
            &args.userId,
        )
        .map_err(ProviderError::Api)?;

        let task = appsmarket_user_license_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
