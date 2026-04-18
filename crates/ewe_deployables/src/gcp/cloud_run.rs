//! GCP Cloud Run Service deployable.
//!
//! WHY: Deploys a container to GCP Cloud Run.
//!
//! WHAT: Deploys via GCP Cloud Run API and persists state.
//!
//! HOW: Implements `Deployable` trait with `ProviderClient` for state and HTTP access.

use foundation_core::valtron::{
    BoxedSendExecutionAction, Stream, StreamIterator, StreamIteratorExt, TaskIterator,
    TaskIteratorExt,
};
use foundation_core::valtron::{execute, ThreadedValue};
use foundation_core::wire::simple_http::client::SystemDnsResolver;
use foundation_db::state::FileStateStore;
use foundation_deployment::provider_client::ProviderClient;
use foundation_deployment::providers::gcp::api::run::RunProvider;
use foundation_deployment::providers::gcp::clients::run::{
    RunProjectsLocationsServicesDeleteArgs, RunProjectsLocationsServicesPatchArgs,
};
use foundation_deployment::traits::{Deployable, Deploying};
use foundation_deployment::types::CloudRunDeployment;

/// GCP Cloud Run service deployable - deploys a container to Cloud Run.
///
/// WHY: Users need a simple way to deploy containers to GCP Cloud Run without writing
///      the deployment logic themselves.
///
/// WHAT: Deploys via GCP Cloud Run API and persists state.
///
/// HOW: Implements `Deployable` trait. Uses `ProviderClient` for state and HTTP access.
#[derive(Debug, Clone)]
pub struct CloudRunService {
    /// Service name.
    pub name: String,
    /// Container image tag.
    pub image: String,
    /// GCP region (e.g., `us-central1`).
    pub region: String,
    /// GCP project ID.
    pub project_id: String,
}

impl CloudRunService {
    /// Create a new Cloud Run service deployable.
    ///
    /// # Arguments
    ///
    /// * `name` - Service name
    /// * `image` - Container image tag (e.g., `gcr.io/project/image:latest`)
    /// * `region` - GCP region
    /// * `project_id` - GCP project ID
    ///
    /// # Example
    ///
    /// ```rust
    /// use ewe_deployables::gcp::CloudRunService;
    /// let service = CloudRunService::new("my-service", "gcr.io/project/image:latest", "us-central1", "project-id");
    /// ```
    pub fn new(
        name: impl Into<String>,
        image: impl Into<String>,
        region: impl Into<String>,
        project_id: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            image: image.into(),
            region: region.into(),
            project_id: project_id.into(),
        }
    }
}

/// Error type for GCP Cloud Run deployments.
#[derive(Debug, thiserror::Error)]
pub enum CloudRunServiceError {
    /// State store error.
    #[error("State store error: {0}")]
    StateFailed(String),

    /// Valtron executor error.
    #[error("Executor error: {0}")]
    ExecutorFailed(String),

    /// GCP API error.
    #[error("API error: {0}")]
    ApiFailed(String),

    /// State deserialization error.
    #[error("Failed to deserialize state: {0}")]
    DeserializeFailed(String),
}

impl Deployable for CloudRunService {
    type DeployOutput = CloudRunDeployment;
    type DestroyOutput = ();
    type Error = CloudRunServiceError;
    type Store = FileStateStore;
    type Resolver = SystemDnsResolver;

    fn deploy(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<
        impl StreamIterator<D = Result<Self::DeployOutput, Self::Error>, P = Deploying> + Send + 'static,
        Self::Error,
    > {
        let task = self.deploy_task(client)?;
        execute(task, None).map_err(|e| CloudRunServiceError::ExecutorFailed(e.to_string()))
    }

    fn deploy_task(
        &self,
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
        let resource_name = format!(
            "projects/{}/locations/{}/services/{}",
            self.project_id, self.region, self.name
        );

        // Create RunProvider from client
        let run = RunProvider::from_provider_client(client);

        let service_name = self.name.clone();
        let region = self.region.clone();
        let project_id = self.project_id.clone();
        let image = self.image.clone();

        // Use generated GCP Cloud Run client
        let result = run
            .run_projects_locations_services_patch(&RunProjectsLocationsServicesPatchArgs {
                name: resource_name.clone(),
                allowMissing: Some("true".to_string()),
                forceNewRevision: Some("true".to_string()),
                updateMask: Some("spec.template.spec.containers".to_string()),
                validateOnly: None,
            })
            .map_err(|e| CloudRunServiceError::ApiFailed(e.to_string()))?;

        Ok(result.map_next(move |api_result| {
            api_result
                .map(|_| {
                    CloudRunDeployment::new(
                        resource_name.clone(),
                        service_name.clone(),
                        region.clone(),
                        project_id.clone(),
                        format!("https://{}-{}.a.run.app", service_name, region),
                        image.clone(),
                    )
                })
                .map_err(|e| CloudRunServiceError::ApiFailed(e.to_string()))
        }))
    }

    fn destroy(
        &self,
        client: ProviderClient<Self::Store, Self::Resolver>,
    ) -> Result<
        impl StreamIterator<D = Result<Self::DestroyOutput, Self::Error>, P = Deploying> + Send + 'static,
        Self::Error,
    > {
        let task = self.destroy_task(client)?;
        execute(task, None).map_err(|e| CloudRunServiceError::ExecutorFailed(e.to_string()))
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
        let resource_id = format!(
            "gcp:cloud-run:{}:{}:{}:{}",
            client.project(),
            self.project_id,
            self.region,
            self.name
        );

        // Read state from store
        let existing_state = state_store
            .get(&resource_id)
            .map_err(|e| {
                CloudRunServiceError::StateFailed(format!(
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
                // Deserialize stored state into typed output
                let output: CloudRunDeployment = serde_json::from_value(state.output.clone())
                    .map_err(|e| {
                        CloudRunServiceError::DeserializeFailed(format!(
                            "Failed to deserialize state: {e}",
                        ))
                    })?;

                // Create RunProvider from client
                let run = RunProvider::from_provider_client(client);

                // Delete service
                let result = run
                    .run_projects_locations_services_delete(
                        &RunProjectsLocationsServicesDeleteArgs {
                            name: output.resource_name,
                            etag: None,
                            validateOnly: None,
                        },
                    )
                    .map_err(|e| CloudRunServiceError::ApiFailed(e.to_string()))?;

                Ok(result.map_next(|api_result| {
                    api_result
                        .map(|_| ())
                        .map_err(|e| CloudRunServiceError::ApiFailed(e.to_string()))
                }))
            }
            None => {
                // No state found - idempotent success
                tracing::warn!(
                    "No state found for Cloud Run service '{}' - skipping destroy (idempotent)",
                    self.name
                );
                Ok(vec![Stream::Next(Ok(()))].into_iter())
            }
        }
    }
}
