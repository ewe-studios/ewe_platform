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
    storagetransfer_projects_agent_pools_create_builder, storagetransfer_projects_agent_pools_create_task,
    storagetransfer_projects_agent_pools_delete_builder, storagetransfer_projects_agent_pools_delete_task,
    storagetransfer_projects_agent_pools_patch_builder, storagetransfer_projects_agent_pools_patch_task,
    storagetransfer_transfer_jobs_create_builder, storagetransfer_transfer_jobs_create_task,
    storagetransfer_transfer_jobs_delete_builder, storagetransfer_transfer_jobs_delete_task,
    storagetransfer_transfer_jobs_patch_builder, storagetransfer_transfer_jobs_patch_task,
    storagetransfer_transfer_jobs_run_builder, storagetransfer_transfer_jobs_run_task,
    storagetransfer_transfer_operations_cancel_builder, storagetransfer_transfer_operations_cancel_task,
    storagetransfer_transfer_operations_pause_builder, storagetransfer_transfer_operations_pause_task,
    storagetransfer_transfer_operations_resume_builder, storagetransfer_transfer_operations_resume_task,
};
use crate::providers::gcp::clients::types::{ApiError, ApiPending};
use crate::providers::gcp::clients::storagetransfer::AgentPool;
use crate::providers::gcp::clients::storagetransfer::Empty;
use crate::providers::gcp::clients::storagetransfer::Operation;
use crate::providers::gcp::clients::storagetransfer::TransferJob;
use crate::providers::gcp::clients::storagetransfer::StoragetransferProjectsAgentPoolsCreateArgs;
use crate::providers::gcp::clients::storagetransfer::StoragetransferProjectsAgentPoolsDeleteArgs;
use crate::providers::gcp::clients::storagetransfer::StoragetransferProjectsAgentPoolsPatchArgs;
use crate::providers::gcp::clients::storagetransfer::StoragetransferTransferJobsCreateArgs;
use crate::providers::gcp::clients::storagetransfer::StoragetransferTransferJobsDeleteArgs;
use crate::providers::gcp::clients::storagetransfer::StoragetransferTransferJobsPatchArgs;
use crate::providers::gcp::clients::storagetransfer::StoragetransferTransferJobsRunArgs;
use crate::providers::gcp::clients::storagetransfer::StoragetransferTransferOperationsCancelArgs;
use crate::providers::gcp::clients::storagetransfer::StoragetransferTransferOperationsPauseArgs;
use crate::providers::gcp::clients::storagetransfer::StoragetransferTransferOperationsResumeArgs;
use crate::provider_client::{ProviderClient, ProviderError};
use foundation_core::valtron::{execute, StreamIterator};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_db::state::store_state_task::StoreStateIdentifierTask;
use std::sync::Arc;

/// StoragetransferProvider with automatic state tracking.
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
/// let provider = StoragetransferProvider::new(client, http_client);
/// ```
#[derive(Clone)]
pub struct StoragetransferProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    client: ProviderClient<S>,
    http_client: Arc<SimpleHttpClient>,
}

impl<S> StoragetransferProvider<S>
where
    S: foundation_db::state::traits::StateStore + Send + Sync + 'static,
{
    /// Create new StoragetransferProvider.
    pub fn new(client: ProviderClient<S>, http_client: SimpleHttpClient) -> Self {
        Self {
            client,
            http_client: Arc::new(http_client),
        }
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagetransfer projects agent pools patch.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
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
            &args.projectId,
        )
        .map_err(ProviderError::Api)?;

        let task = storagetransfer_transfer_jobs_delete_task(builder)
            .map_err(ProviderError::Api)?;

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagetransfer transfer jobs patch.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagetransfer transfer jobs run.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagetransfer transfer operations cancel.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagetransfer transfer operations pause.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

    /// Storagetransfer transfer operations resume.
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

        let state_store = self.client.state_store.clone();
        let stage = Some(self.client.stage.clone());

        let store_task = StoreStateIdentifierTask::new(task, state_store, args, stage);

        execute(store_task, None).map_err(|e: String| ProviderError::ExecuteFailed(e.to_string()))
    }

}
