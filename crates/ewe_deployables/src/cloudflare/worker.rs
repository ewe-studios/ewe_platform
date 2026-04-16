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
use foundation_core::wire::simple_http::client::{DnsResolver, SystemDnsResolver};
use foundation_db::state::traits::StateStore;
use foundation_db::state::FileStateStore;
use foundation_deployment::provider_client::ProviderClient;
use foundation_deployment::providers::cloudflare::api::cloudflare::CloudflareProvider;
use foundation_deployment::providers::cloudflare::clients::cloudflare::{
    WorkerScriptDeleteWorkerArgs,
    WorkerScriptUploadWorkerModuleArgs,
    worker_script_delete_worker,
    worker_script_upload_worker_module,
};
use foundation_deployment::traits::{Deployable, Deploying};
use foundation_deployment::types::WorkerDeployment;
use serde::{Deserialize, Serialize};

use std::fmt::Debug;

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
    type Output = WorkerDeployment;
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
        self.deploy_task(client)
            .and_then(|task| execute(task, None))
            .map_err(|e| CloudflareWorkerError::ExecutorFailed(e.to_string()))
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
        let script = std::fs::read_to_string(&self.script_path).map_err(|e| {
            CloudflareWorkerError::IoFailed(format!(
                "Failed to read script from {}: {}",
                self.script_path, e
            ))
        })?;

        // Create CloudflareProvider from client
        let cloudflare = CloudflareProvider::from_provider_client(client);

        // Use generated Cloudflare client
        let result = cloudflare
            .worker_script_upload_worker_module(&WorkerScriptUploadWorkerModuleArgs {
                account_id: self.account_id.clone(),
                script_name: self.name.clone(),
                bindings_inherit: Some(true),
                body: serde_json::json!({
                    "main": script,
                }),
            })
            .map_err(|e| CloudflareWorkerError::ApiFailed(e.to_string()))?;

        // Transform API response into deployment output
        Ok(result.map(|response| WorkerDeployment {
            account_id: self.account_id.clone(),
            script_name: self.name.clone(),
            deployment_id: response.result.id.clone(),
            url: format!("https://{}.workers.dev", self.name),
            deployed_at: chrono::Utc::now(),
        }))
    }

    fn destroy(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<
        impl StreamIterator<D = Result<(), Self::Error>, P = Deploying> + Send + 'static,
        Self::Error,
    > {
        self.destroy_task(client)
            .and_then(|task| execute(task, None))
            .map_err(|e| CloudflareWorkerError::ExecutorFailed(e.to_string()))
    }

    fn destroy_task(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<
        impl TaskIterator<
                Ready = Result<(), Self::Error>,
                Pending = Deploying,
                Spawner = BoxedSendExecutionAction,
            > + Send
            + 'static,
        Self::Error,
    > {
        let state_store = client.state_store();
        let resource_id = format!("cloudflare:worker:{}:{}", client.project(), self.name);

        // Read state from store
        let existing_state = state_store
            .get(&resource_id)
            .map_err(|e| {
                CloudflareWorkerError::StateFailed(format!(
                    "Failed to get state for {}: {}",
                    resource_id, e
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
                // Deserialize stored state into typed output
                let output: WorkerDeployment = serde_json::from_value(state.output.clone())
                    .map_err(|e| {
                        CloudflareWorkerError::DeserializeFailed(format!(
                            "Failed to deserialize state: {}",
                            e
                        ))
                    })?;

                // Create CloudflareProvider from client
                let cloudflare = CloudflareProvider::from_provider_client(client);

                // Delete worker
                let result = cloudflare
                    .worker_script_delete_worker(&WorkerScriptDeleteWorkerArgs {
                        account_id: output.account_id,
                        script_name: output.script_name,
                        force: None,
                    })
                    .map_err(|e| CloudflareWorkerError::ApiFailed(e.to_string()))?;

                Ok(result.map(|_| ()))
            }
            None => {
                // No state found - idempotent success
                tracing::warn!(
                    "No state found for worker '{}' - skipping destroy (idempotent)",
                    self.name
                );
                Ok(Box::new(std::iter::once(StreamIterator::Next(Ok(())))))
            }
        }
    }
}
