//! WebriskProvider - State-aware webrisk API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       webrisk API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::webrisk::{
    webrisk_hashes_search_builder, webrisk_hashes_search_task,
    webrisk_projects_operations_cancel_builder, webrisk_projects_operations_cancel_task,
    webrisk_projects_operations_delete_builder, webrisk_projects_operations_delete_task,
    webrisk_projects_operations_get_builder, webrisk_projects_operations_get_task,
    webrisk_projects_operations_list_builder, webrisk_projects_operations_list_task,
    webrisk_projects_submissions_create_builder, webrisk_projects_submissions_create_task,
    webrisk_threat_lists_compute_diff_builder, webrisk_threat_lists_compute_diff_task,
    webrisk_uris_search_builder, webrisk_uris_search_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::webrisk::GoogleCloudWebriskV1ComputeThreatListDiffResponse;
use crate::providers::gcp::clients::webrisk::GoogleCloudWebriskV1SearchHashesResponse;
use crate::providers::gcp::clients::webrisk::GoogleCloudWebriskV1SearchUrisResponse;
use crate::providers::gcp::clients::webrisk::GoogleCloudWebriskV1Submission;
use crate::providers::gcp::clients::webrisk::GoogleLongrunningListOperationsResponse;
use crate::providers::gcp::clients::webrisk::GoogleLongrunningOperation;
use crate::providers::gcp::clients::webrisk::GoogleProtobufEmpty;
use crate::providers::gcp::clients::webrisk::WebriskHashesSearchArgs;
use crate::providers::gcp::clients::webrisk::WebriskProjectsOperationsCancelArgs;
use crate::providers::gcp::clients::webrisk::WebriskProjectsOperationsDeleteArgs;
use crate::providers::gcp::clients::webrisk::WebriskProjectsOperationsGetArgs;
use crate::providers::gcp::clients::webrisk::WebriskProjectsOperationsListArgs;
use crate::providers::gcp::clients::webrisk::WebriskProjectsSubmissionsCreateArgs;
use crate::providers::gcp::clients::webrisk::WebriskThreatListsComputeDiffArgs;
use crate::providers::gcp::clients::webrisk::WebriskUrisSearchArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// WebriskProvider with automatic state tracking.
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
/// let provider = WebriskProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct WebriskProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> WebriskProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new WebriskProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new WebriskProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Webrisk hashes search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudWebriskV1SearchHashesResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn webrisk_hashes_search(
        &self,
        args: &WebriskHashesSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudWebriskV1SearchHashesResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = webrisk_hashes_search_builder(
            &self.http_client,
            &args.hashPrefix,
            &args.threatTypes,
        )
        .map_err(ProviderError::Api)?;

        let task = webrisk_hashes_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Webrisk projects operations cancel.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn webrisk_projects_operations_cancel(
        &self,
        args: &WebriskProjectsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = webrisk_projects_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = webrisk_projects_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Webrisk projects operations delete.
    ///
    /// Automatically stores the result in the state store on success.
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
    /// Returns ProviderError if the API request or state storage fails.
    pub fn webrisk_projects_operations_delete(
        &self,
        args: &WebriskProjectsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleProtobufEmpty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = webrisk_projects_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = webrisk_projects_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Webrisk projects operations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningOperation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn webrisk_projects_operations_get(
        &self,
        args: &WebriskProjectsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningOperation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = webrisk_projects_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = webrisk_projects_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Webrisk projects operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleLongrunningListOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn webrisk_projects_operations_list(
        &self,
        args: &WebriskProjectsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleLongrunningListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = webrisk_projects_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = webrisk_projects_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Webrisk projects submissions create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudWebriskV1Submission result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn webrisk_projects_submissions_create(
        &self,
        args: &WebriskProjectsSubmissionsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudWebriskV1Submission, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = webrisk_projects_submissions_create_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = webrisk_projects_submissions_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Webrisk threat lists compute diff.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudWebriskV1ComputeThreatListDiffResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn webrisk_threat_lists_compute_diff(
        &self,
        args: &WebriskThreatListsComputeDiffArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudWebriskV1ComputeThreatListDiffResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = webrisk_threat_lists_compute_diff_builder(
            &self.http_client,
            &args.constraints_maxDatabaseEntries,
            &args.constraints_maxDiffEntries,
            &args.constraints_supportedCompressions,
            &args.threatType,
            &args.versionToken,
        )
        .map_err(ProviderError::Api)?;

        let task = webrisk_threat_lists_compute_diff_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Webrisk uris search.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleCloudWebriskV1SearchUrisResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn webrisk_uris_search(
        &self,
        args: &WebriskUrisSearchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleCloudWebriskV1SearchUrisResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = webrisk_uris_search_builder(
            &self.http_client,
            &args.threatTypes,
            &args.uri,
        )
        .map_err(ProviderError::Api)?;

        let task = webrisk_uris_search_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
