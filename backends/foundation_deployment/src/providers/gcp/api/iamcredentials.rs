//! IamcredentialsProvider - State-aware iamcredentials API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       iamcredentials API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::iamcredentials::{
    iamcredentials_locations_workforce_pools_get_allowed_locations_builder, iamcredentials_locations_workforce_pools_get_allowed_locations_task,
    iamcredentials_projects_locations_workload_identity_pools_get_allowed_locations_builder, iamcredentials_projects_locations_workload_identity_pools_get_allowed_locations_task,
    iamcredentials_projects_service_accounts_generate_access_token_builder, iamcredentials_projects_service_accounts_generate_access_token_task,
    iamcredentials_projects_service_accounts_generate_id_token_builder, iamcredentials_projects_service_accounts_generate_id_token_task,
    iamcredentials_projects_service_accounts_get_allowed_locations_builder, iamcredentials_projects_service_accounts_get_allowed_locations_task,
    iamcredentials_projects_service_accounts_sign_blob_builder, iamcredentials_projects_service_accounts_sign_blob_task,
    iamcredentials_projects_service_accounts_sign_jwt_builder, iamcredentials_projects_service_accounts_sign_jwt_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::iamcredentials::GenerateAccessTokenResponse;
use crate::providers::gcp::clients::iamcredentials::GenerateIdTokenResponse;
use crate::providers::gcp::clients::iamcredentials::ServiceAccountAllowedLocations;
use crate::providers::gcp::clients::iamcredentials::SignBlobResponse;
use crate::providers::gcp::clients::iamcredentials::SignJwtResponse;
use crate::providers::gcp::clients::iamcredentials::WorkforcePoolAllowedLocations;
use crate::providers::gcp::clients::iamcredentials::WorkloadIdentityPoolAllowedLocations;
use crate::providers::gcp::clients::iamcredentials::IamcredentialsLocationsWorkforcePoolsGetAllowedLocationsArgs;
use crate::providers::gcp::clients::iamcredentials::IamcredentialsProjectsLocationsWorkloadIdentityPoolsGetAllowedLocationsArgs;
use crate::providers::gcp::clients::iamcredentials::IamcredentialsProjectsServiceAccountsGenerateAccessTokenArgs;
use crate::providers::gcp::clients::iamcredentials::IamcredentialsProjectsServiceAccountsGenerateIdTokenArgs;
use crate::providers::gcp::clients::iamcredentials::IamcredentialsProjectsServiceAccountsGetAllowedLocationsArgs;
use crate::providers::gcp::clients::iamcredentials::IamcredentialsProjectsServiceAccountsSignBlobArgs;
use crate::providers::gcp::clients::iamcredentials::IamcredentialsProjectsServiceAccountsSignJwtArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// IamcredentialsProvider with automatic state tracking.
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
/// let provider = IamcredentialsProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct IamcredentialsProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> IamcredentialsProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new IamcredentialsProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new IamcredentialsProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Iamcredentials locations workforce pools get allowed locations.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WorkforcePoolAllowedLocations result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn iamcredentials_locations_workforce_pools_get_allowed_locations(
        &self,
        args: &IamcredentialsLocationsWorkforcePoolsGetAllowedLocationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WorkforcePoolAllowedLocations, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iamcredentials_locations_workforce_pools_get_allowed_locations_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = iamcredentials_locations_workforce_pools_get_allowed_locations_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Iamcredentials projects locations workload identity pools get allowed locations.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the WorkloadIdentityPoolAllowedLocations result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn iamcredentials_projects_locations_workload_identity_pools_get_allowed_locations(
        &self,
        args: &IamcredentialsProjectsLocationsWorkloadIdentityPoolsGetAllowedLocationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<WorkloadIdentityPoolAllowedLocations, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iamcredentials_projects_locations_workload_identity_pools_get_allowed_locations_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = iamcredentials_projects_locations_workload_identity_pools_get_allowed_locations_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Iamcredentials projects service accounts generate access token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenerateAccessTokenResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn iamcredentials_projects_service_accounts_generate_access_token(
        &self,
        args: &IamcredentialsProjectsServiceAccountsGenerateAccessTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenerateAccessTokenResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iamcredentials_projects_service_accounts_generate_access_token_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = iamcredentials_projects_service_accounts_generate_access_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Iamcredentials projects service accounts generate id token.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GenerateIdTokenResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn iamcredentials_projects_service_accounts_generate_id_token(
        &self,
        args: &IamcredentialsProjectsServiceAccountsGenerateIdTokenArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GenerateIdTokenResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iamcredentials_projects_service_accounts_generate_id_token_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = iamcredentials_projects_service_accounts_generate_id_token_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Iamcredentials projects service accounts get allowed locations.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ServiceAccountAllowedLocations result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn iamcredentials_projects_service_accounts_get_allowed_locations(
        &self,
        args: &IamcredentialsProjectsServiceAccountsGetAllowedLocationsArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ServiceAccountAllowedLocations, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iamcredentials_projects_service_accounts_get_allowed_locations_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = iamcredentials_projects_service_accounts_get_allowed_locations_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Iamcredentials projects service accounts sign blob.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SignBlobResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn iamcredentials_projects_service_accounts_sign_blob(
        &self,
        args: &IamcredentialsProjectsServiceAccountsSignBlobArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SignBlobResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iamcredentials_projects_service_accounts_sign_blob_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = iamcredentials_projects_service_accounts_sign_blob_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Iamcredentials projects service accounts sign jwt.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the SignJwtResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn iamcredentials_projects_service_accounts_sign_jwt(
        &self,
        args: &IamcredentialsProjectsServiceAccountsSignJwtArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<SignJwtResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = iamcredentials_projects_service_accounts_sign_jwt_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = iamcredentials_projects_service_accounts_sign_jwt_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
