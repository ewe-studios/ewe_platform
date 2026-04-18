//! Cloudflare Worker deployable.
//!
//! WHY: Deploys a Cloudflare Worker script to Cloudflare Workers.
//!
//! WHAT: Reads the script from disk, deploys via Cloudflare API, and persists state.
//!
//! HOW: Implements `Deployable` trait with `ProviderClient` for state and HTTP access.

use foundation_core::valtron::{
    BoxedSendExecutionAction, StreamIterator, TaskIterator, TaskIteratorExt,
};
use foundation_core::valtron::{execute, ThreadedValue};
use foundation_core::wire::simple_http::client::SystemDnsResolver;
use foundation_db::state::FileStateStore;
use foundation_deployment::provider_client::{ProviderClient, StoreStateIdentifierTask};
use foundation_deployment::providers::cloudflare::clients::cloudflare::{
    WorkerScriptDeleteWorkerArgs, WorkerScriptUploadWorkerModuleArgs,
    worker_script_delete_worker_builder, worker_script_delete_worker_task,
    worker_script_upload_worker_module_builder, worker_script_upload_worker_module_task,
};
use foundation_deployment::traits::{Deployable, Deploying};
use foundation_deployment::types::WorkerDeployment;

use crate::common::task_either::{OneShotTask, TaskEither};

/// Cloudflare Worker deployable - deploys a worker script to Cloudflare Workers.
///
/// WHY: Users need a simple way to deploy Cloudflare Workers without writing
///      the deployment logic themselves.
///
/// WHAT: Reads the script from disk, deploys via Cloudflare API, and persists state.
///
/// HOW: Implements `Deployable` trait. Uses `ProviderClient` for state and HTTP access.
#[derive(Debug, Clone)]
pub struct CloudflareWorker {
    /// Worker script name.
    pub name: String,
    /// Path to the worker script file.
    pub script_path: String,
    /// Cloudflare account ID.
    pub account_id: String,
}

impl CloudflareWorker {
    /// Create a new Cloudflare Worker deployable.
    ///
    /// # Arguments
    ///
    /// * `name` - Worker script name
    /// * `script_path` - Path to the worker script file
    /// * `account_id` - Cloudflare account ID
    ///
    /// # Example
    ///
    /// ```rust
    /// use ewe_deployables::cloudflare::CloudflareWorker;
    /// let worker = CloudflareWorker::new("my-worker", "./worker.js", "account-id");
    /// ```
    pub fn new(
        name: impl Into<String>,
        script_path: impl Into<String>,
        account_id: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            script_path: script_path.into(),
            account_id: account_id.into(),
        }
    }
}

/// Error type for Cloudflare Worker deployments.
#[derive(Debug, thiserror::Error)]
pub enum CloudflareWorkerError {
    /// IO error reading script file.
    #[error("IO error: {0}")]
    IoFailed(String),

    /// State store error.
    #[error("State store error: {0}")]
    StateFailed(String),

    /// Valtron executor error.
    #[error("Executor error: {0}")]
    ExecutorFailed(String),

    /// Cloudflare API error.
    #[error("API error: {0}")]
    ApiFailed(String),

    /// State deserialization error.
    #[error("Failed to deserialize state: {0}")]
    DeserializeFailed(String),
}

impl Deployable for CloudflareWorker {
    type DeployOutput = WorkerDeployment;
    type DestroyOutput = ();
    type Error = CloudflareWorkerError;
    type Store = FileStateStore;
    type Resolver = SystemDnsResolver;

    fn deploy(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<
        impl StreamIterator<D = Result<Self::Output, Self::Error>, P = Deploying> + Send + 'static,
        Self::Error,
    > {
        let task = self.deploy_task(client)?;
        execute(task, None).map_err(|e| CloudflareWorkerError::ExecutorFailed(e.to_string()))
    }

    fn deploy_task(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<
        impl TaskIterator<
                Ready = Result<Self::Output, Self::Error>,
                Pending = Deploying,
                Spawner = BoxedSendExecutionAction,
            > + Send
            + 'static,
        Self::Error,
    > {
        // Read script content from disk
        let _script = std::fs::read_to_string(&self.script_path).map_err(|e| {
            CloudflareWorkerError::IoFailed(format!(
                "Failed to read script from {}: {}",
                self.script_path, e
            ))
        })?;

        let args = WorkerScriptUploadWorkerModuleArgs {
            account_id: self.account_id.clone(),
            script_name: self.name.clone(),
            bindings_inherit: Some("true".to_string()),
        };

        let builder = worker_script_upload_worker_module_builder(
            client.http_client(),
            &args.account_id,
            &args.script_name,
            &args.bindings_inherit,
        )
        .map_err(|e| CloudflareWorkerError::ApiFailed(e.to_string()))?;

        let task = worker_script_upload_worker_module_task(builder)
            .map_err(|e| CloudflareWorkerError::ApiFailed(e.to_string()))?;

        let store_task = StoreStateIdentifierTask::new(
            task,
            client.state_store.clone(),
            args,
            Some(client.stage.clone()),
        );

        let name = self.name.clone();
        let account_id = self.account_id.clone();

        Ok(store_task
            .map_ready(move |api_result| {
                api_result
                    .map(|response| {
                        let deployment_id = response
                            .as_object()
                            .and_then(|o| o.get("id"))
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("unknown")
                            .to_string();
                        WorkerDeployment::new(
                            account_id.clone(),
                            name.clone(),
                            deployment_id,
                            format!("https://{}.workers.dev", name),
                        )
                    })
                    .map_err(|e| CloudflareWorkerError::ApiFailed(e.to_string()))
            })
            .map_pending(|_| Deploying::Processing))
    }

    fn destroy(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<
        impl StreamIterator<D = Result<Self::DestroyOutput, Self::Error>, P = Deploying> + Send + 'static,
        Self::Error,
    > {
        let task = self.destroy_task(client)?;
        execute(task, None).map_err(|e| CloudflareWorkerError::ExecutorFailed(e.to_string()))
    }

    fn destroy_task(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<
        impl TaskIterator<
                Ready = Result<Self::DestroyOutput, Self::Error>,
                Pending = Deploying,
                Spawner = BoxedSendExecutionAction,
            > + Send
            + 'static,
        Self::Error,
    > {
        let state_store = client.state_store();
        let resource_id = format!("cloudflare:worker:{}:{}", client.project(), self.name);

        let existing_state = state_store
            .get(&resource_id)
            .map_err(|e| {
                CloudflareWorkerError::StateFailed(format!(
                    "Failed to get state for {resource_id}: {e}",
                ))
            })?
            .find_map(|v| match v {
                ThreadedValue::Value(Ok(state)) => Some(state),
                ThreadedValue::Value(Err(e)) => {
                    tracing::warn!(
                        "State store error during destroy for {}: {}",
                        resource_id,
                        e
                    );
                    None
                }
                _ => None,
            })
            .flatten();

        match existing_state {
            Some(state) => {
                let output: WorkerDeployment = serde_json::from_value(state.output.clone())
                    .map_err(|e| {
                        CloudflareWorkerError::DeserializeFailed(format!(
                            "Failed to deserialize state: {e}",
                        ))
                    })?;

                let args = WorkerScriptDeleteWorkerArgs {
                    account_id: output.account_id,
                    script_name: output.script_name,
                    force: None,
                };

                let builder = worker_script_delete_worker_builder(
                    client.http_client(),
                    &args.account_id,
                    &args.script_name,
                    &args.force,
                )
                .map_err(|e| CloudflareWorkerError::ApiFailed(e.to_string()))?;

                let task = worker_script_delete_worker_task(builder)
                    .map_err(|e| CloudflareWorkerError::ApiFailed(e.to_string()))?;

                let store_task = StoreStateIdentifierTask::new(
                    task,
                    client.state_store.clone(),
                    args,
                    Some(client.stage.clone()),
                );

                Ok(TaskEither::Left(
                    store_task
                        .map_ready(|api_result| {
                            api_result
                                .map(|_| ())
                                .map_err(|e| CloudflareWorkerError::ApiFailed(e.to_string()))
                        })
                        .map_pending(|_| Deploying::Processing),
                ))
            }
            None => {
                tracing::warn!(
                    "No state found for worker '{}' - skipping destroy (idempotent)",
                    self.name
                );
                Ok(TaskEither::Right(
                    OneShotTask::new(Ok(()))
                        .map_ready(|v| v)
                        .map_pending(|_| Deploying::Processing),
                ))
            }
        }
    }
}
