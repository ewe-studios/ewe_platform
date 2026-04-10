//! AddressvalidationProvider - State-aware addressvalidation API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       addressvalidation API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::addressvalidation::{
    addressvalidation_provide_validation_feedback_builder, addressvalidation_provide_validation_feedback_task,
    addressvalidation_validate_address_builder, addressvalidation_validate_address_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::addressvalidation::GoogleMapsAddressvalidationV1ProvideValidationFeedbackResponse;
use crate::providers::gcp::clients::addressvalidation::GoogleMapsAddressvalidationV1ValidateAddressResponse;
use crate::providers::gcp::clients::addressvalidation::AddressvalidationProvideValidationFeedbackArgs;
use crate::providers::gcp::clients::addressvalidation::AddressvalidationValidateAddressArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// AddressvalidationProvider with automatic state tracking.
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
/// let provider = AddressvalidationProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct AddressvalidationProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> AddressvalidationProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new AddressvalidationProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Addressvalidation provide validation feedback.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleMapsAddressvalidationV1ProvideValidationFeedbackResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn addressvalidation_provide_validation_feedback(
        &self,
        args: &AddressvalidationProvideValidationFeedbackArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleMapsAddressvalidationV1ProvideValidationFeedbackResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = addressvalidation_provide_validation_feedback_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = addressvalidation_provide_validation_feedback_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Addressvalidation validate address.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleMapsAddressvalidationV1ValidateAddressResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn addressvalidation_validate_address(
        &self,
        args: &AddressvalidationValidateAddressArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleMapsAddressvalidationV1ValidateAddressResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = addressvalidation_validate_address_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = addressvalidation_validate_address_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
