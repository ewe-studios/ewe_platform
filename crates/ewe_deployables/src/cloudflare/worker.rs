//! Cloudflare Worker deployable.
//!
//! WHY: Deploys a Cloudflare Worker script to Cloudflare Workers.
//!
//! WHAT: Reads the script from disk, deploys via Cloudflare API, and persists state.
//!
//! HOW: Implements `Deployable` trait with `ProviderClient` for state and HTTP access.

use foundation_core::valtron::{BoxedSendExecutionAction, TaskIterator, TaskIteratorExt};
use foundation_core::wire::simple_http::client::{DnsResolver, SystemDnsResolver};
use foundation_db::state::FileStateStore;
use foundation_deployment::provider_client::ProviderClient;
use foundation_deployment::providers::cloudflare::workers::{
    worker_script_delete_worker_request, worker_script_upload_worker_module_request,
    WorkerScriptDeleteWorkerArgs, WorkerScriptUploadWorkerModuleArgs,
};
use foundation_deployment::traits::{Deployable, Deploying};
use foundation_deployment::types::WorkerDeployment;

use std::marker::PhantomData;

/// Cloudflare Worker deployable - deploys a worker script to Cloudflare Workers.
#[derive(Debug, Clone)]
pub struct CloudflareWorker<R = SystemDnsResolver> {
    /// Worker script name.
    pub name: String,
    /// Path to the worker script file.
    pub script_path: String,
    /// Cloudflare account ID.
    pub account_id: String,
    _resolver: PhantomData<R>,
}

impl<R> CloudflareWorker<R> {
    /// Create a new Cloudflare Worker deployable.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        script_path: impl Into<String>,
        account_id: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            script_path: script_path.into(),
            account_id: account_id.into(),
            _resolver: PhantomData,
        }
    }
}

/// Error type for Cloudflare Worker deployments.
#[derive(Debug, thiserror::Error)]
pub enum CloudflareWorkerError {
    /// IO error reading script file.
    #[error("IO error reading '{path}': {source}")]
    IoError {
        path: String,
        source: std::io::Error,
    },

    /// Cloudflare API error.
    #[error("API error: {0}")]
    ApiError(String),
}

impl<R: DnsResolver + Clone + Default + 'static> Deployable for CloudflareWorker<R> {
    const NAMESPACE: &'static str = "cloudflare/workers/script";

    type DeployOutput = WorkerDeployment;
    type DestroyOutput = ();
    type Error = CloudflareWorkerError;
    type Store = FileStateStore;
    type Resolver = R;

    fn deploy(
        &self,
        instance_id: usize,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<
        impl TaskIterator<
                Ready = Result<Self::DeployOutput, Self::Error>,
                Pending = Deploying,
                Spawner = BoxedSendExecutionAction,
            > + Send
            + 'static,
        Self::Error,
    > {
        // Read script from disk
        let _script = std::fs::read_to_string(&self.script_path).map_err(|e| {
            CloudflareWorkerError::IoError {
                path: self.script_path.clone(),
                source: e,
            }
        })?;

        let args = WorkerScriptUploadWorkerModuleArgs {
            account_id: self.account_id.clone(),
            script_name: self.name.clone(),
            bindings_inherit: None,
        };

        let task = worker_script_upload_worker_module_request(
            client.http_client(),
            &args,
            None::<fn(&mut _)>,
        )
        .map_err(|e| CloudflareWorkerError::ApiError(e.to_string()))?;

        let name = self.name.clone();
        let account_id = self.account_id.clone();

        let task = task
            .map_ready(move |api_result| {
                api_result
                    .map(|response| {
                        let deployment_id = response
                            .body
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
                    .map_err(|e| CloudflareWorkerError::ApiError(e.to_string()))
            })
            .map_pending(|_| Deploying::Processing);

        Ok(self.update(&client, instance_id, &args, task))
    }

    fn destroy(
        &self,
        instance_id: usize,
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
        let store = self.store(&client);
        let state: WorkerDeployment = store
            .get_typed(&instance_id.to_string())
            .map_err(|e| CloudflareWorkerError::ApiError(format!("Failed to read state: {e}")))?
            .ok_or_else(|| {
                CloudflareWorkerError::ApiError(format!(
                    "No state found for worker '{}' instance {instance_id} — nothing to destroy",
                    self.name
                ))
            })?;

        let args = WorkerScriptDeleteWorkerArgs {
            account_id: state.account_id,
            script_name: state.script_name,
            force: None,
        };

        let task =
            worker_script_delete_worker_request(client.http_client(), &args, None::<fn(&mut _)>)
                .map_err(|e| CloudflareWorkerError::ApiError(e.to_string()))?;

        Ok(task
            .map_ready(|api_result| {
                api_result
                    .map(|_| ())
                    .map_err(|e| CloudflareWorkerError::ApiError(e.to_string()))
            })
            .map_pending(|_| Deploying::Processing))
    }
}
