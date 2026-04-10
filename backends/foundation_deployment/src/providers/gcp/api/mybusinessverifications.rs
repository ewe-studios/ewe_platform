//! MybusinessverificationsProvider - State-aware mybusinessverifications API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       mybusinessverifications API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::mybusinessverifications::{
    mybusinessverifications_locations_fetch_verification_options_builder, mybusinessverifications_locations_fetch_verification_options_task,
    mybusinessverifications_locations_get_voice_of_merchant_state_builder, mybusinessverifications_locations_get_voice_of_merchant_state_task,
    mybusinessverifications_locations_verify_builder, mybusinessverifications_locations_verify_task,
    mybusinessverifications_locations_verifications_complete_builder, mybusinessverifications_locations_verifications_complete_task,
    mybusinessverifications_locations_verifications_list_builder, mybusinessverifications_locations_verifications_list_task,
    mybusinessverifications_verification_tokens_generate_builder, mybusinessverifications_verification_tokens_generate_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::mybusinessverifications::CompleteVerificationResponse;
use crate::providers::gcp::clients::mybusinessverifications::FetchVerificationOptionsResponse;
use crate::providers::gcp::clients::mybusinessverifications::GenerateInstantVerificationTokenResponse;
use crate::providers::gcp::clients::mybusinessverifications::ListVerificationsResponse;
use crate::providers::gcp::clients::mybusinessverifications::VerifyLocationResponse;
use crate::providers::gcp::clients::mybusinessverifications::VoiceOfMerchantState;
use crate::providers::gcp::clients::mybusinessverifications::MybusinessverificationsLocationsFetchVerificationOptionsArgs;
use crate::providers::gcp::clients::mybusinessverifications::MybusinessverificationsLocationsGetVoiceOfMerchantStateArgs;
use crate::providers::gcp::clients::mybusinessverifications::MybusinessverificationsLocationsVerificationsCompleteArgs;
use crate::providers::gcp::clients::mybusinessverifications::MybusinessverificationsLocationsVerificationsListArgs;
use crate::providers::gcp::clients::mybusinessverifications::MybusinessverificationsLocationsVerifyArgs;
use crate::providers::gcp::clients::mybusinessverifications::MybusinessverificationsVerificationTokensGenerateArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// MybusinessverificationsProvider with automatic state tracking.
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
/// let provider = MybusinessverificationsProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct MybusinessverificationsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> MybusinessverificationsProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new MybusinessverificationsProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
    }

    /// Mybusinessverifications locations fetch verification options.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the FetchVerificationOptionsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn mybusinessverifications_locations_fetch_verification_options(
        &self,
        args: &MybusinessverificationsLocationsFetchVerificationOptionsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<FetchVerificationOptionsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessverifications_locations_fetch_verification_options_builder(
            &self.http_client,
            &args.location,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessverifications_locations_fetch_verification_options_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessverifications locations get voice of merchant state.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VoiceOfMerchantState result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn mybusinessverifications_locations_get_voice_of_merchant_state(
        &self,
        args: &MybusinessverificationsLocationsGetVoiceOfMerchantStateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VoiceOfMerchantState, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessverifications_locations_get_voice_of_merchant_state_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessverifications_locations_get_voice_of_merchant_state_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessverifications locations verify.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the VerifyLocationResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn mybusinessverifications_locations_verify(
        &self,
        args: &MybusinessverificationsLocationsVerifyArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<VerifyLocationResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessverifications_locations_verify_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessverifications_locations_verify_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessverifications locations verifications complete.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the CompleteVerificationResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn mybusinessverifications_locations_verifications_complete(
        &self,
        args: &MybusinessverificationsLocationsVerificationsCompleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<CompleteVerificationResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessverifications_locations_verifications_complete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessverifications_locations_verifications_complete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessverifications locations verifications list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListVerificationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn mybusinessverifications_locations_verifications_list(
        &self,
        args: &MybusinessverificationsLocationsVerificationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListVerificationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessverifications_locations_verifications_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessverifications_locations_verifications_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Mybusinessverifications verification tokens generate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenerateInstantVerificationTokenResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn mybusinessverifications_verification_tokens_generate(
        &self,
        args: &MybusinessverificationsVerificationTokensGenerateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenerateInstantVerificationTokenResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = mybusinessverifications_verification_tokens_generate_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = mybusinessverifications_verification_tokens_generate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
