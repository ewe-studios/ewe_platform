//! ManufacturersProvider - State-aware manufacturers API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       manufacturers API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::manufacturers::{
    manufacturers_accounts_languages_product_certifications_delete_builder, manufacturers_accounts_languages_product_certifications_delete_task,
    manufacturers_accounts_languages_product_certifications_get_builder, manufacturers_accounts_languages_product_certifications_get_task,
    manufacturers_accounts_languages_product_certifications_list_builder, manufacturers_accounts_languages_product_certifications_list_task,
    manufacturers_accounts_languages_product_certifications_patch_builder, manufacturers_accounts_languages_product_certifications_patch_task,
    manufacturers_accounts_products_delete_builder, manufacturers_accounts_products_delete_task,
    manufacturers_accounts_products_get_builder, manufacturers_accounts_products_get_task,
    manufacturers_accounts_products_list_builder, manufacturers_accounts_products_list_task,
    manufacturers_accounts_products_update_builder, manufacturers_accounts_products_update_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::manufacturers::Empty;
use crate::providers::gcp::clients::manufacturers::ListProductCertificationsResponse;
use crate::providers::gcp::clients::manufacturers::ListProductsResponse;
use crate::providers::gcp::clients::manufacturers::Product;
use crate::providers::gcp::clients::manufacturers::ProductCertification;
use crate::providers::gcp::clients::manufacturers::ManufacturersAccountsLanguagesProductCertificationsDeleteArgs;
use crate::providers::gcp::clients::manufacturers::ManufacturersAccountsLanguagesProductCertificationsGetArgs;
use crate::providers::gcp::clients::manufacturers::ManufacturersAccountsLanguagesProductCertificationsListArgs;
use crate::providers::gcp::clients::manufacturers::ManufacturersAccountsLanguagesProductCertificationsPatchArgs;
use crate::providers::gcp::clients::manufacturers::ManufacturersAccountsProductsDeleteArgs;
use crate::providers::gcp::clients::manufacturers::ManufacturersAccountsProductsGetArgs;
use crate::providers::gcp::clients::manufacturers::ManufacturersAccountsProductsListArgs;
use crate::providers::gcp::clients::manufacturers::ManufacturersAccountsProductsUpdateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ManufacturersProvider with automatic state tracking.
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
/// let provider = ManufacturersProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct ManufacturersProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> ManufacturersProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new ManufacturersProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new ManufacturersProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Manufacturers accounts languages product certifications delete.
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
    pub fn manufacturers_accounts_languages_product_certifications_delete(
        &self,
        args: &ManufacturersAccountsLanguagesProductCertificationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = manufacturers_accounts_languages_product_certifications_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = manufacturers_accounts_languages_product_certifications_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Manufacturers accounts languages product certifications get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProductCertification result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn manufacturers_accounts_languages_product_certifications_get(
        &self,
        args: &ManufacturersAccountsLanguagesProductCertificationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProductCertification, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = manufacturers_accounts_languages_product_certifications_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = manufacturers_accounts_languages_product_certifications_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Manufacturers accounts languages product certifications list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListProductCertificationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn manufacturers_accounts_languages_product_certifications_list(
        &self,
        args: &ManufacturersAccountsLanguagesProductCertificationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListProductCertificationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = manufacturers_accounts_languages_product_certifications_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = manufacturers_accounts_languages_product_certifications_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Manufacturers accounts languages product certifications patch.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ProductCertification result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn manufacturers_accounts_languages_product_certifications_patch(
        &self,
        args: &ManufacturersAccountsLanguagesProductCertificationsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ProductCertification, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = manufacturers_accounts_languages_product_certifications_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = manufacturers_accounts_languages_product_certifications_patch_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Manufacturers accounts products delete.
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
    pub fn manufacturers_accounts_products_delete(
        &self,
        args: &ManufacturersAccountsProductsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = manufacturers_accounts_products_delete_builder(
            &self.http_client,
            &args.parent,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = manufacturers_accounts_products_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Manufacturers accounts products get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Product result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn manufacturers_accounts_products_get(
        &self,
        args: &ManufacturersAccountsProductsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Product, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = manufacturers_accounts_products_get_builder(
            &self.http_client,
            &args.parent,
            &args.name,
            &args.include,
        )
        .map_err(ProviderError::Api)?;

        let task = manufacturers_accounts_products_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Manufacturers accounts products list.
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
    pub fn manufacturers_accounts_products_list(
        &self,
        args: &ManufacturersAccountsProductsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListProductsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = manufacturers_accounts_products_list_builder(
            &self.http_client,
            &args.parent,
            &args.include,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = manufacturers_accounts_products_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Manufacturers accounts products update.
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
    pub fn manufacturers_accounts_products_update(
        &self,
        args: &ManufacturersAccountsProductsUpdateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = manufacturers_accounts_products_update_builder(
            &self.http_client,
            &args.parent,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = manufacturers_accounts_products_update_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
