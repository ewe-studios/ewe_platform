//! GCP Cloud Run Service deployable.
//!
//! WHY: Deploys a container to GCP Cloud Run.
//!
//! WHAT: Deploys via GCP Cloud Run API and persists state.
//!
//! HOW: Implements `Deployable` trait with `ProviderClient` for state and HTTP access.

use foundation_core::valtron::{BoxedSendExecutionAction, TaskIterator, TaskIteratorExt};
use foundation_core::wire::simple_http::client::{DnsResolver, SystemDnsResolver};
use foundation_db::state::FileStateStore;
use foundation_deployment::provider_client::ProviderClient;
use foundation_deployment::providers::gcp::run::run::{
    run_projects_locations_jobs_create_request, run_projects_locations_jobs_delete_request,
    run_projects_locations_services_delete_request,
    run_projects_locations_services_patch_request,
    GoogleCloudRunV2Container, GoogleCloudRunV2ExecutionTemplate, GoogleCloudRunV2Job,
    GoogleCloudRunV2RevisionTemplate, GoogleCloudRunV2Service, GoogleCloudRunV2TaskTemplate,
    RunProjectsLocationsJobsCreateArgs, RunProjectsLocationsJobsDeleteArgs,
    RunProjectsLocationsServicesDeleteArgs, RunProjectsLocationsServicesPatchArgs,
};
use foundation_deployment::traits::{Deployable, Deploying};
use foundation_deployment::types::{CloudRunDeployment, CloudRunJobDeployment};

use std::marker::PhantomData;

// ---------------------------------------------------------------------------
// CloudRunService
// ---------------------------------------------------------------------------

/// GCP Cloud Run service deployable - deploys a container to Cloud Run.
#[derive(Debug, Clone)]
pub struct CloudRunService<R = SystemDnsResolver> {
    /// Service name.
    pub name: String,
    /// Container image tag.
    pub image: String,
    /// GCP region (e.g., `us-central1`).
    pub region: String,
    /// GCP project ID.
    pub project_id: String,
    _resolver: PhantomData<R>,
}

impl<R> CloudRunService<R> {
    /// Create a new Cloud Run service deployable.
    #[must_use]
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
            _resolver: PhantomData,
        }
    }
}

/// Error type for GCP Cloud Run deployments.
#[derive(Debug, thiserror::Error)]
pub enum CloudRunServiceError {
    /// State store error.
    #[error("State store error: {0}")]
    StateFailed(String),

    /// GCP API error.
    #[error("API error: {0}")]
    ApiFailed(String),
}

impl<R: DnsResolver + Clone + Default + 'static> Deployable for CloudRunService<R> {
    const NAMESPACE: &'static str = "gcp/cloud-run/service";

    type DeployOutput = CloudRunDeployment;
    type DestroyOutput = ();
    type Error = CloudRunServiceError;
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
        let resource_name = format!(
            "projects/{}/locations/{}/services/{}",
            self.project_id, self.region, self.name
        );

        let args = RunProjectsLocationsServicesPatchArgs {
            name: resource_name.clone(),
            allow_missing: Some("true".to_string()),
            force_new_revision: Some("true".to_string()),
            update_mask: Some("spec.template.spec.containers".to_string()),
            validate_only: None,
            body: GoogleCloudRunV2Service {
                template: Some(GoogleCloudRunV2RevisionTemplate {
                    containers: Some(vec![GoogleCloudRunV2Container {
                        image: Some(self.image.clone()),
                        ..Default::default()
                    }]),
                    ..Default::default()
                }),
                ..Default::default()
            },
        };

        let task = run_projects_locations_services_patch_request(
            client.http_client(),
            &args,
            None::<fn(&mut _)>,
        )
        .map_err(|e| CloudRunServiceError::ApiFailed(e.to_string()))?;

        let service_name = self.name.clone();
        let region = self.region.clone();
        let project_id = self.project_id.clone();
        let image = self.image.clone();
        let resource_name_clone = resource_name.clone();

        let task = task
            .map_ready(move |api_result| {
                api_result
                    .map(|_| {
                        CloudRunDeployment::new(
                            resource_name_clone.clone(),
                            service_name.clone(),
                            region.clone(),
                            project_id.clone(),
                            format!("https://{}-{}.a.run.app", service_name, region),
                            image.clone(),
                        )
                    })
                    .map_err(|e| CloudRunServiceError::ApiFailed(e.to_string()))
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
        let state: CloudRunDeployment = store
            .get_typed(&instance_id.to_string())
            .map_err(|e| {
                CloudRunServiceError::StateFailed(format!("Failed to read state: {e}"))
            })?
            .ok_or_else(|| {
                CloudRunServiceError::StateFailed(format!(
                    "No state found for service '{}' instance {instance_id} — nothing to destroy",
                    self.name
                ))
            })?;

        let args = RunProjectsLocationsServicesDeleteArgs {
            name: state.resource_name,
            etag: None,
            validate_only: None,
        };

        let task = run_projects_locations_services_delete_request(
            client.http_client(),
            &args,
            None::<fn(&mut _)>,
        )
        .map_err(|e| CloudRunServiceError::ApiFailed(e.to_string()))?;

        Ok(task
            .map_ready(|api_result| {
                api_result
                    .map(|_| ())
                    .map_err(|e| CloudRunServiceError::ApiFailed(e.to_string()))
            })
            .map_pending(|_| Deploying::Processing))
    }
}

// ---------------------------------------------------------------------------
// CloudRunJob
// ---------------------------------------------------------------------------

/// GCP Cloud Run Job deployable - creates/schedules a Cloud Run Job.
#[derive(Debug, Clone)]
pub struct CloudRunJob<R = SystemDnsResolver> {
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
    _resolver: PhantomData<R>,
}

impl<R> CloudRunJob<R> {
    /// Create a new Cloud Run Job deployable.
    #[must_use]
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
            _resolver: PhantomData,
        }
    }

    /// Set the cron schedule for this job.
    #[must_use]
    pub fn with_schedule(mut self, schedule: impl Into<String>) -> Self {
        self.schedule = Some(schedule.into());
        self
    }

    /// Set the command to run in the job container.
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

    /// GCP API error.
    #[error("API error: {0}")]
    ApiFailed(String),
}

impl<R: DnsResolver + Clone + Default + 'static> Deployable for CloudRunJob<R> {
    const NAMESPACE: &'static str = "gcp/cloud-run/job";

    type DeployOutput = CloudRunJobDeployment;
    type DestroyOutput = ();
    type Error = CloudRunJobError;
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
        let parent = format!("projects/{}/locations/{}", self.project_id, self.region);

        let args = RunProjectsLocationsJobsCreateArgs {
            parent: parent.clone(),
            job_id: Some(self.name.clone()),
            validate_only: None,
            body: GoogleCloudRunV2Job {
                template: Some(GoogleCloudRunV2ExecutionTemplate {
                    template: Some(GoogleCloudRunV2TaskTemplate {
                        containers: Some(vec![GoogleCloudRunV2Container {
                            image: Some(self.image.clone()),
                            command: self.command.clone(),
                            ..Default::default()
                        }]),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            },
        };

        let task = run_projects_locations_jobs_create_request(
            client.http_client(),
            &args,
            None::<fn(&mut _)>,
        )
        .map_err(|e| CloudRunJobError::ApiFailed(e.to_string()))?;

        let job_name = self.name.clone();
        let region = self.region.clone();
        let project_id = self.project_id.clone();
        let parent_clone = parent.clone();

        let task = task
            .map_ready(move |api_result| {
                api_result
                    .map(|_| {
                        CloudRunJobDeployment::new(
                            format!("{}/jobs/{}", parent_clone, job_name),
                            job_name.clone(),
                            region.clone(),
                            project_id.clone(),
                        )
                    })
                    .map_err(|e| CloudRunJobError::ApiFailed(e.to_string()))
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
        let state: CloudRunJobDeployment = store
            .get_typed(&instance_id.to_string())
            .map_err(|e| {
                CloudRunJobError::StateFailed(format!("Failed to read state: {e}"))
            })?
            .ok_or_else(|| {
                CloudRunJobError::StateFailed(format!(
                    "No state found for job '{}' instance {instance_id} — nothing to destroy",
                    self.name
                ))
            })?;

        let args = RunProjectsLocationsJobsDeleteArgs {
            name: state.resource_name,
            etag: None,
            validate_only: None,
        };

        let task = run_projects_locations_jobs_delete_request(
            client.http_client(),
            &args,
            None::<fn(&mut _)>,
        )
        .map_err(|e| CloudRunJobError::ApiFailed(e.to_string()))?;

        Ok(task
            .map_ready(|api_result| {
                api_result
                    .map(|_| ())
                    .map_err(|e| CloudRunJobError::ApiFailed(e.to_string()))
            })
            .map_pending(|_| Deploying::Processing))
    }
}
