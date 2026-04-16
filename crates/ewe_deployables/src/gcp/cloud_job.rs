//! GCP Cloud Run Job deployable.
//!
//! WHY: Deploys a container job to GCP Cloud Run Jobs.
//!
//! WHAT: Deploys via GCP Cloud Run API and persists state.
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
use foundation_deployment::providers::gcp::api::run::RunProvider;
use foundation_deployment::providers::gcp::clients::run::{
    RunProjectsLocationsJobsCreateArgs,
    RunProjectsLocationsJobsDeleteArgs,
    run_projects_locations_jobs_create,
    run_projects_locations_jobs_delete,
};
use foundation_deployment::traits::{Deployable, Deploying};
use foundation_deployment::types::CloudRunJobDeployment;
use serde::{Deserialize, Serialize};

use std::fmt::Debug;

/// GCP Cloud Run Job deployable - creates/schedules a Cloud Run Job.
///
/// WHY: Users need a simple way to deploy container jobs to GCP Cloud Run Jobs.
///
/// WHAT: Deploys via GCP Cloud Run API and persists state.
///
/// HOW: Implements `Deployable` trait. Uses `ProviderClient` for state and HTTP access.
#[derive(Debug, Clone)]
pub struct CloudRunJob {
    /// Job name.
    pub name: String,
    /// Container image tag.
    pub image: String,
    /// GCP region (e.g., `us-central1`).
    pub region: String,
    /// GCP project ID.
    pub project_id: String,
    /// Optional cron schedule.
    pub schedule: Option<String>,
    /// Optional command to run in the job container.
    pub command: Option<Vec<String>>,
}

impl CloudRunJob {
    /// Create a new Cloud Run Job deployable.
    ///
    /// # Arguments
    ///
    /// * `name` - Job name
    /// * `image` - Container image tag (e.g., `gcr.io/project/image:latest`)
    /// * `region` - GCP region
    /// * `project_id` - GCP project ID
    ///
    /// # Example
    ///
    /// ```rust
    /// let job = CloudRunJob::new("my-job", "gcr.io/project/image:latest", "us-central1", "project-id");
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
            schedule: None,
            command: None,
        }
    }

    /// Set the cron schedule for this job.
    ///
    /// # Arguments
    ///
    /// * `schedule` - Cron schedule (e.g., `"0 * * * *"` for hourly)
    #[must_use]
    pub fn with_schedule(mut self, schedule: impl Into<String>) -> Self {
        self.schedule = Some(schedule.into());
        self
    }

    /// Set the command to run in the job container.
    ///
    /// # Arguments
    ///
    /// * `command` - Command and arguments to run
    #[must_use]
    pub fn with_command(mut self, command: Vec<String>) -> Self {
        self.command = Some(command);
        self
    }
}

/// Error type for GCP Cloud Run Job deployments.
#[derive(Debug, thiserror::Error)]
pub enum CloudRunJobError {
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

impl Deployable for CloudRunJob {
    type Output = CloudRunJobDeployment;
    type Error = CloudRunJobError;
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
            .map_err(|e| CloudRunJobError::ExecutorFailed(e.to_string()))
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
        let run = RunProvider::from_provider_client(client);
        let parent = format!("projects/{}/locations/{}", self.project_id, self.region);

        // Build job spec
        let mut job_spec = serde_json::json!({
            "template": {
                "spec": {
                    "containers": [{
                        "image": self.image,
                    }]
                }
            }
        });

        // Add optional command
        if let Some(ref cmd) = self.command {
            job_spec["template"]["spec"]["containers"][0]["command"] = serde_json::json!(cmd);
        }

        // Add optional schedule
        if let Some(ref schedule) = self.schedule {
            job_spec["schedule"] = serde_json::json!({ "cron": schedule });
        }

        // Use generated GCP Cloud Run client
        let result = run
            .run_projects_locations_jobs_create(&RunProjectsLocationsJobsCreateArgs {
                parent,
                job_id: self.name.clone(),
                body: job_spec,
                validate_only: None,
            })
            .map_err(|e| CloudRunJobError::ApiFailed(e.to_string()))?;

        Ok(result.map(|_| CloudRunJobDeployment {
            resource_name: format!(
                "projects/{}/locations/{}/jobs/{}",
                self.project_id, self.region, self.name
            ),
            job_name: self.name.clone(),
            region: self.region.clone(),
            project_id: self.project_id.clone(),
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
            .map_err(|e| CloudRunJobError::ExecutorFailed(e.to_string()))
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
        let resource_id = format!(
            "gcp:cloud-job:{}:{}:{}:{}",
            client.project(),
            self.project_id,
            self.region,
            self.name
        );

        // Read state from store
        let existing_state = state_store
            .get(&resource_id)
            .map_err(|e| {
                CloudRunJobError::StateFailed(format!(
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
                let output: CloudRunJobDeployment = serde_json::from_value(state.output.clone())
                    .map_err(|e| {
                        CloudRunJobError::DeserializeFailed(format!(
                            "Failed to deserialize state: {}",
                            e
                        ))
                    })?;

                // Create RunProvider from client
                let run = RunProvider::from_provider_client(client);

                // Delete job
                let result = run
                    .run_projects_locations_jobs_delete(&RunProjectsLocationsJobsDeleteArgs {
                        name: output.resource_name,
                        etag: None,
                        validate_only: None,
                    })
                    .map_err(|e| CloudRunJobError::ApiFailed(e.to_string()))?;

                Ok(result.map(|_| ()))
            }
            None => {
                // No state found - idempotent success
                tracing::warn!(
                    "No state found for Cloud Run Job '{}' - skipping destroy (idempotent)",
                    self.name
                );
                Ok(Box::new(std::iter::once(StreamIterator::Next(Ok(())))))
            }
        }
    }
}
