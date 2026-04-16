//! ChecksProvider - State-aware checks API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       checks API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::checks::{
    checks_accounts_apps_get_builder, checks_accounts_apps_get_task,
    checks_accounts_apps_list_builder, checks_accounts_apps_list_task,
    checks_accounts_apps_operations_cancel_builder, checks_accounts_apps_operations_cancel_task,
    checks_accounts_apps_operations_delete_builder, checks_accounts_apps_operations_delete_task,
    checks_accounts_apps_operations_get_builder, checks_accounts_apps_operations_get_task,
    checks_accounts_apps_operations_list_builder, checks_accounts_apps_operations_list_task,
    checks_accounts_apps_operations_wait_builder, checks_accounts_apps_operations_wait_task,
    checks_accounts_apps_reports_get_builder, checks_accounts_apps_reports_get_task,
    checks_accounts_apps_reports_list_builder, checks_accounts_apps_reports_list_task,
    checks_accounts_repos_operations_get_builder, checks_accounts_repos_operations_get_task,
    checks_accounts_repos_scans_generate_builder, checks_accounts_repos_scans_generate_task,
    checks_accounts_repos_scans_get_builder, checks_accounts_repos_scans_get_task,
    checks_accounts_repos_scans_list_builder, checks_accounts_repos_scans_list_task,
    checks_aisafety_classify_content_builder, checks_aisafety_classify_content_task,
    checks_media_upload_builder, checks_media_upload_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::checks::Empty;
use crate::providers::gcp::clients::checks::GoogleChecksAccountV1alphaApp;
use crate::providers::gcp::clients::checks::GoogleChecksAccountV1alphaListAppsResponse;
use crate::providers::gcp::clients::checks::GoogleChecksAisafetyV1alphaClassifyContentResponse;
use crate::providers::gcp::clients::checks::GoogleChecksRepoScanV1alphaListRepoScansResponse;
use crate::providers::gcp::clients::checks::GoogleChecksRepoScanV1alphaRepoScan;
use crate::providers::gcp::clients::checks::GoogleChecksReportV1alphaListReportsResponse;
use crate::providers::gcp::clients::checks::GoogleChecksReportV1alphaReport;
use crate::providers::gcp::clients::checks::ListOperationsResponse;
use crate::providers::gcp::clients::checks::Operation;
use crate::providers::gcp::clients::checks::ChecksAccountsAppsGetArgs;
use crate::providers::gcp::clients::checks::ChecksAccountsAppsListArgs;
use crate::providers::gcp::clients::checks::ChecksAccountsAppsOperationsCancelArgs;
use crate::providers::gcp::clients::checks::ChecksAccountsAppsOperationsDeleteArgs;
use crate::providers::gcp::clients::checks::ChecksAccountsAppsOperationsGetArgs;
use crate::providers::gcp::clients::checks::ChecksAccountsAppsOperationsListArgs;
use crate::providers::gcp::clients::checks::ChecksAccountsAppsOperationsWaitArgs;
use crate::providers::gcp::clients::checks::ChecksAccountsAppsReportsGetArgs;
use crate::providers::gcp::clients::checks::ChecksAccountsAppsReportsListArgs;
use crate::providers::gcp::clients::checks::ChecksAccountsReposOperationsGetArgs;
use crate::providers::gcp::clients::checks::ChecksAccountsReposScansGenerateArgs;
use crate::providers::gcp::clients::checks::ChecksAccountsReposScansGetArgs;
use crate::providers::gcp::clients::checks::ChecksAccountsReposScansListArgs;
use crate::providers::gcp::clients::checks::ChecksMediaUploadArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// ChecksProvider with automatic state tracking.
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
/// let provider = ChecksProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct ChecksProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> ChecksProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new ChecksProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new ChecksProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Checks accounts apps get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChecksAccountV1alphaApp result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn checks_accounts_apps_get(
        &self,
        args: &ChecksAccountsAppsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChecksAccountV1alphaApp, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = checks_accounts_apps_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = checks_accounts_apps_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Checks accounts apps list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChecksAccountV1alphaListAppsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn checks_accounts_apps_list(
        &self,
        args: &ChecksAccountsAppsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChecksAccountV1alphaListAppsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = checks_accounts_apps_list_builder(
            &self.http_client,
            &args.parent,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = checks_accounts_apps_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Checks accounts apps operations cancel.
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
    pub fn checks_accounts_apps_operations_cancel(
        &self,
        args: &ChecksAccountsAppsOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = checks_accounts_apps_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = checks_accounts_apps_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Checks accounts apps operations delete.
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
    pub fn checks_accounts_apps_operations_delete(
        &self,
        args: &ChecksAccountsAppsOperationsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = checks_accounts_apps_operations_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = checks_accounts_apps_operations_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Checks accounts apps operations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn checks_accounts_apps_operations_get(
        &self,
        args: &ChecksAccountsAppsOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = checks_accounts_apps_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = checks_accounts_apps_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Checks accounts apps operations list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListOperationsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn checks_accounts_apps_operations_list(
        &self,
        args: &ChecksAccountsAppsOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = checks_accounts_apps_operations_list_builder(
            &self.http_client,
            &args.name,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = checks_accounts_apps_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Checks accounts apps operations wait.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn checks_accounts_apps_operations_wait(
        &self,
        args: &ChecksAccountsAppsOperationsWaitArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = checks_accounts_apps_operations_wait_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = checks_accounts_apps_operations_wait_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Checks accounts apps reports get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChecksReportV1alphaReport result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn checks_accounts_apps_reports_get(
        &self,
        args: &ChecksAccountsAppsReportsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChecksReportV1alphaReport, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = checks_accounts_apps_reports_get_builder(
            &self.http_client,
            &args.name,
            &args.checksFilter,
        )
        .map_err(ProviderError::Api)?;

        let task = checks_accounts_apps_reports_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Checks accounts apps reports list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChecksReportV1alphaListReportsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn checks_accounts_apps_reports_list(
        &self,
        args: &ChecksAccountsAppsReportsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChecksReportV1alphaListReportsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = checks_accounts_apps_reports_list_builder(
            &self.http_client,
            &args.parent,
            &args.checksFilter,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = checks_accounts_apps_reports_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Checks accounts repos operations get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn checks_accounts_repos_operations_get(
        &self,
        args: &ChecksAccountsReposOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = checks_accounts_repos_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = checks_accounts_repos_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Checks accounts repos scans generate.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn checks_accounts_repos_scans_generate(
        &self,
        args: &ChecksAccountsReposScansGenerateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = checks_accounts_repos_scans_generate_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = checks_accounts_repos_scans_generate_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Checks accounts repos scans get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChecksRepoScanV1alphaRepoScan result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn checks_accounts_repos_scans_get(
        &self,
        args: &ChecksAccountsReposScansGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChecksRepoScanV1alphaRepoScan, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = checks_accounts_repos_scans_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = checks_accounts_repos_scans_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Checks accounts repos scans list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChecksRepoScanV1alphaListRepoScansResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn checks_accounts_repos_scans_list(
        &self,
        args: &ChecksAccountsReposScansListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChecksRepoScanV1alphaListRepoScansResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = checks_accounts_repos_scans_list_builder(
            &self.http_client,
            &args.parent,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = checks_accounts_repos_scans_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Checks aisafety classify content.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleChecksAisafetyV1alphaClassifyContentResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn checks_aisafety_classify_content(
        &self,
        args: &ChecksAisafetyClassifyContentArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleChecksAisafetyV1alphaClassifyContentResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = checks_aisafety_classify_content_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = checks_aisafety_classify_content_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Checks media upload.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the Operation result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn checks_media_upload(
        &self,
        args: &ChecksMediaUploadArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = checks_media_upload_builder(
            &self.http_client,
            &args.parent,
        )
        .map_err(ProviderError::Api)?;

        let task = checks_media_upload_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
