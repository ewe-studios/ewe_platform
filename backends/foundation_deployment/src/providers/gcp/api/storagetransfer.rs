//! StoragetransferProvider - State-aware storagetransfer API client.
//!
//! WHY: Users need state-aware API clients that automatically track
//!      resource changes in the state store.
//!
//! WHAT: Provider wrapping ProviderClient<S> with methods for
//!       storagetransfer API endpoints that auto-store results.
//!
//! HOW: Each method wraps the task with StoreStateIdentifierTask
//!      for automatic state persistence on success.

#![cfg(feature = "gcp")]

use crate::providers::gcp::clients::storagetransfer::{
    storagetransfer_google_service_accounts_get_builder, storagetransfer_google_service_accounts_get_task,
    storagetransfer_projects_agent_pools_create_builder, storagetransfer_projects_agent_pools_create_task,
    storagetransfer_projects_agent_pools_delete_builder, storagetransfer_projects_agent_pools_delete_task,
    storagetransfer_projects_agent_pools_get_builder, storagetransfer_projects_agent_pools_get_task,
    storagetransfer_projects_agent_pools_list_builder, storagetransfer_projects_agent_pools_list_task,
    storagetransfer_projects_agent_pools_patch_builder, storagetransfer_projects_agent_pools_patch_task,
    storagetransfer_transfer_jobs_create_builder, storagetransfer_transfer_jobs_create_task,
    storagetransfer_transfer_jobs_delete_builder, storagetransfer_transfer_jobs_delete_task,
    storagetransfer_transfer_jobs_get_builder, storagetransfer_transfer_jobs_get_task,
    storagetransfer_transfer_jobs_list_builder, storagetransfer_transfer_jobs_list_task,
    storagetransfer_transfer_jobs_patch_builder, storagetransfer_transfer_jobs_patch_task,
    storagetransfer_transfer_jobs_run_builder, storagetransfer_transfer_jobs_run_task,
    storagetransfer_transfer_operations_cancel_builder, storagetransfer_transfer_operations_cancel_task,
    storagetransfer_transfer_operations_get_builder, storagetransfer_transfer_operations_get_task,
    storagetransfer_transfer_operations_list_builder, storagetransfer_transfer_operations_list_task,
    storagetransfer_transfer_operations_pause_builder, storagetransfer_transfer_operations_pause_task,
    storagetransfer_transfer_operations_resume_builder, storagetransfer_transfer_operations_resume_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::storagetransfer::AgentPool;
use crate::providers::gcp::clients::storagetransfer::Empty;
use crate::providers::gcp::clients::storagetransfer::GoogleServiceAccount;
use crate::providers::gcp::clients::storagetransfer::ListAgentPoolsResponse;
use crate::providers::gcp::clients::storagetransfer::ListOperationsResponse;
use crate::providers::gcp::clients::storagetransfer::ListTransferJobsResponse;
use crate::providers::gcp::clients::storagetransfer::Operation;
use crate::providers::gcp::clients::storagetransfer::TransferJob;
use crate::providers::gcp::clients::storagetransfer::StoragetransferGoogleServiceAccountsGetArgs;
use crate::providers::gcp::clients::storagetransfer::StoragetransferProjectsAgentPoolsCreateArgs;
use crate::providers::gcp::clients::storagetransfer::StoragetransferProjectsAgentPoolsDeleteArgs;
use crate::providers::gcp::clients::storagetransfer::StoragetransferProjectsAgentPoolsGetArgs;
use crate::providers::gcp::clients::storagetransfer::StoragetransferProjectsAgentPoolsListArgs;
use crate::providers::gcp::clients::storagetransfer::StoragetransferProjectsAgentPoolsPatchArgs;
use crate::providers::gcp::clients::storagetransfer::StoragetransferTransferJobsDeleteArgs;
use crate::providers::gcp::clients::storagetransfer::StoragetransferTransferJobsGetArgs;
use crate::providers::gcp::clients::storagetransfer::StoragetransferTransferJobsListArgs;
use crate::providers::gcp::clients::storagetransfer::StoragetransferTransferJobsPatchArgs;
use crate::providers::gcp::clients::storagetransfer::StoragetransferTransferJobsRunArgs;
use crate::providers::gcp::clients::storagetransfer::StoragetransferTransferOperationsCancelArgs;
use crate::providers::gcp::clients::storagetransfer::StoragetransferTransferOperationsGetArgs;
use crate::providers::gcp::clients::storagetransfer::StoragetransferTransferOperationsListArgs;
use crate::providers::gcp::clients::storagetransfer::StoragetransferTransferOperationsPauseArgs;
use crate::providers::gcp::clients::storagetransfer::StoragetransferTransferOperationsResumeArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::{SimpleHttpClient, DnsResolver};
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// StoragetransferProvider with automatic state tracking.
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
/// let provider = StoragetransferProvider::from_provider_client(client);
/// ```
#[derive(Clone)]
pub struct StoragetransferProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    client: ProviderClient<S, R>,
    http_client: Arc<SimpleHttpClient<R>>,
}

impl<S, R> StoragetransferProvider<S, R>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
    R: foundation_core::wire::simple_http::client::DnsResolver + Clone + 'static,
{
    /// Create new StoragetransferProvider.
    pub fn new(client: ProviderClient<S, R>, http_client: Arc<SimpleHttpClient<R>>) -> Self {
        Self {
            client,
            http_client,
        }
    }

    /// Create new StoragetransferProvider from ProviderClient, extracting the HTTP client.
    ///
    /// This is a convenience method that calls `Self::new()` with `client.http_client()`.
    pub fn from_provider_client(client: ProviderClient<S, R>) -> Self {
        Self::new(client, client.http_client.clone())
    }

    /// Storagetransfer google service accounts get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the GoogleServiceAccount result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn storagetransfer_google_service_accounts_get(
        &self,
        args: &StoragetransferGoogleServiceAccountsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<GoogleServiceAccount, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagetransfer_google_service_accounts_get_builder(
            &self.http_client,
            &args.projectId,
        )
        .map_err(ProviderError::Api)?;

        let task = storagetransfer_google_service_accounts_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagetransfer projects agent pools create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AgentPool result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storagetransfer_projects_agent_pools_create(
        &self,
        args: &StoragetransferProjectsAgentPoolsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AgentPool, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagetransfer_projects_agent_pools_create_builder(
            &self.http_client,
            &args.projectId,
            &args.agentPoolId,
        )
        .map_err(ProviderError::Api)?;

        let task = storagetransfer_projects_agent_pools_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagetransfer projects agent pools delete.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn storagetransfer_projects_agent_pools_delete(
        &self,
        args: &StoragetransferProjectsAgentPoolsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagetransfer_projects_agent_pools_delete_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = storagetransfer_projects_agent_pools_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagetransfer projects agent pools get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AgentPool result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn storagetransfer_projects_agent_pools_get(
        &self,
        args: &StoragetransferProjectsAgentPoolsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AgentPool, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagetransfer_projects_agent_pools_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = storagetransfer_projects_agent_pools_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagetransfer projects agent pools list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListAgentPoolsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn storagetransfer_projects_agent_pools_list(
        &self,
        args: &StoragetransferProjectsAgentPoolsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListAgentPoolsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagetransfer_projects_agent_pools_list_builder(
            &self.http_client,
            &args.projectId,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = storagetransfer_projects_agent_pools_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagetransfer projects agent pools patch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the AgentPool result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn storagetransfer_projects_agent_pools_patch(
        &self,
        args: &StoragetransferProjectsAgentPoolsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<AgentPool, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagetransfer_projects_agent_pools_patch_builder(
            &self.http_client,
            &args.name,
            &args.updateMask,
        )
        .map_err(ProviderError::Api)?;

        let task = storagetransfer_projects_agent_pools_patch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagetransfer transfer jobs create.
    ///
    /// Automatically stores the result in the state store on success.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TransferJob result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request or state storage fails.
    pub fn storagetransfer_transfer_jobs_create(
        &self,
        args: &StoragetransferTransferJobsCreateArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TransferJob, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagetransfer_transfer_jobs_create_builder(
            &self.http_client,
        )
        .map_err(ProviderError::Api)?;

        let task = storagetransfer_transfer_jobs_create_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagetransfer transfer jobs delete.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn storagetransfer_transfer_jobs_delete(
        &self,
        args: &StoragetransferTransferJobsDeleteArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagetransfer_transfer_jobs_delete_builder(
            &self.http_client,
            &args.jobName,
            &args.projectId,
        )
        .map_err(ProviderError::Api)?;

        let task = storagetransfer_transfer_jobs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagetransfer transfer jobs get.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TransferJob result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn storagetransfer_transfer_jobs_get(
        &self,
        args: &StoragetransferTransferJobsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TransferJob, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagetransfer_transfer_jobs_get_builder(
            &self.http_client,
            &args.jobName,
            &args.projectId,
        )
        .map_err(ProviderError::Api)?;

        let task = storagetransfer_transfer_jobs_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagetransfer transfer jobs list.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the ListTransferJobsResponse result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn storagetransfer_transfer_jobs_list(
        &self,
        args: &StoragetransferTransferJobsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListTransferJobsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagetransfer_transfer_jobs_list_builder(
            &self.http_client,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
        )
        .map_err(ProviderError::Api)?;

        let task = storagetransfer_transfer_jobs_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagetransfer transfer jobs patch.
    ///
    /// Read-only operation - no state tracking.
    ///
    /// # Arguments
    ///
    /// * `args` - Request arguments
    ///
    /// # Returns
    ///
    /// StreamIterator yielding the TransferJob result.
    ///
    /// # Errors
    ///
    /// Returns ProviderError if the API request fails.
    pub fn storagetransfer_transfer_jobs_patch(
        &self,
        args: &StoragetransferTransferJobsPatchArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<TransferJob, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagetransfer_transfer_jobs_patch_builder(
            &self.http_client,
            &args.jobName,
        )
        .map_err(ProviderError::Api)?;

        let task = storagetransfer_transfer_jobs_patch_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagetransfer transfer jobs run.
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
    pub fn storagetransfer_transfer_jobs_run(
        &self,
        args: &StoragetransferTransferJobsRunArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagetransfer_transfer_jobs_run_builder(
            &self.http_client,
            &args.jobName,
        )
        .map_err(ProviderError::Api)?;

        let task = storagetransfer_transfer_jobs_run_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagetransfer transfer operations cancel.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn storagetransfer_transfer_operations_cancel(
        &self,
        args: &StoragetransferTransferOperationsCancelArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagetransfer_transfer_operations_cancel_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = storagetransfer_transfer_operations_cancel_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagetransfer transfer operations get.
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
    pub fn storagetransfer_transfer_operations_get(
        &self,
        args: &StoragetransferTransferOperationsGetArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Operation, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagetransfer_transfer_operations_get_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = storagetransfer_transfer_operations_get_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagetransfer transfer operations list.
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
    pub fn storagetransfer_transfer_operations_list(
        &self,
        args: &StoragetransferTransferOperationsListArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<ListOperationsResponse, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagetransfer_transfer_operations_list_builder(
            &self.http_client,
            &args.filter,
            &args.pageSize,
            &args.pageToken,
            &args.returnPartialSuccess,
        )
        .map_err(ProviderError::Api)?;

        let task = storagetransfer_transfer_operations_list_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagetransfer transfer operations pause.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn storagetransfer_transfer_operations_pause(
        &self,
        args: &StoragetransferTransferOperationsPauseArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagetransfer_transfer_operations_pause_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = storagetransfer_transfer_operations_pause_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagetransfer transfer operations resume.
    ///
    /// Read-only operation - no state tracking.
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
    /// Returns ProviderError if the API request fails.
    pub fn storagetransfer_transfer_operations_resume(
        &self,
        args: &StoragetransferTransferOperationsResumeArgs,
    ) -> Result<
        impl StreamIterator<
            D = Result<Empty, ProviderError<ApiError>>,
            P = crate::providers::gcp::clients::types::ApiPending,
        > + Send
        + 'static,
        ProviderError<ApiError>,
    > {
        let builder = storagetransfer_transfer_operations_resume_builder(
            &self.http_client,
            &args.name,
        )
        .map_err(ProviderError::Api)?;

        let task = storagetransfer_transfer_operations_resume_task(builder)
            .map_err(ProviderError::Api)?;

        execute(task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
