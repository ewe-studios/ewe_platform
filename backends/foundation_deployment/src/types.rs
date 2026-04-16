//! Common deployment output types for trait-based deployments.
//!
//! WHY: Deployable implementations need common output types that describe
//!      deployed resources (URLs, IDs, timestamps). These types are stored
//!      in the state store and used during destroy operations.
//!
//! WHAT: Types for Cloudflare Workers, GCP Cloud Run services, GCP Cloud Run Jobs,
//!       and a generic deployment output type.
//!
//! HOW: Plain data structs with `Debug + Clone + Serialize + Deserialize`.
//!      Each type contains the minimum information needed to identify and
//!      destroy the deployed resource.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Cloudflare Worker deployment output.
///
/// WHY: Stores the minimal information needed to identify and destroy a Worker.
///
/// WHAT: Contains account ID, script name, deployment ID, and URL.
///
/// HOW: Returned by `CloudflareWorker::deploy()` and stored in state store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerDeployment {
    /// Cloudflare account ID.
    pub account_id: String,
    /// Worker script name.
    pub script_name: String,
    /// Deployment ID returned by Cloudflare API.
    pub deployment_id: String,
    /// Worker URL (e.g., `https://my-worker.workers.dev`).
    pub url: String,
    /// Timestamp of the deployment.
    pub deployed_at: DateTime<Utc>,
}

impl WorkerDeployment {
    /// Create a new WorkerDeployment.
    ///
    /// # Arguments
    ///
    /// * `account_id` - Cloudflare account ID
    /// * `script_name` - Worker script name
    /// * `deployment_id` - Deployment ID from API
    /// * `url` - Worker URL
    #[must_use]
    pub fn new(
        account_id: impl Into<String>,
        script_name: impl Into<String>,
        deployment_id: impl Into<String>,
        url: impl Into<String>,
    ) -> Self {
        Self {
            account_id: account_id.into(),
            script_name: script_name.into(),
            deployment_id: deployment_id.into(),
            url: url.into(),
            deployed_at: Utc::now(),
        }
    }
}

/// GCP Cloud Run service deployment output.
///
/// WHY: Stores the minimal information needed to identify and destroy a Cloud Run service.
///
/// WHAT: Contains resource name, service name, region, project ID, and URL.
///
/// HOW: Returned by `CloudRunService::deploy()` and stored in state store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudRunDeployment {
    /// Full resource path (e.g., `projects/my-project/locations/us-central1/services/my-service`).
    pub resource_name: String,
    /// Service name.
    pub service_name: String,
    /// GCP region (e.g., `us-central1`).
    pub region: String,
    /// GCP project ID.
    pub project_id: String,
    /// Service URL (e.g., `https://my-service-us-central1.a.run.app`).
    pub url: String,
    /// Container image tag.
    pub image: String,
    /// Timestamp of the deployment.
    pub deployed_at: DateTime<Utc>,
}

impl CloudRunDeployment {
    /// Create a new CloudRunDeployment.
    ///
    /// # Arguments
    ///
    /// * `resource_name` - Full resource path
    /// * `service_name` - Service name
    /// * `region` - GCP region
    /// * `project_id` - GCP project ID
    /// * `url` - Service URL
    /// * `image` - Container image tag
    #[must_use]
    pub fn new(
        resource_name: impl Into<String>,
        service_name: impl Into<String>,
        region: impl Into<String>,
        project_id: impl Into<String>,
        url: impl Into<String>,
        image: impl Into<String>,
    ) -> Self {
        Self {
            resource_name: resource_name.into(),
            service_name: service_name.into(),
            region: region.into(),
            project_id: project_id.into(),
            url: url.into(),
            image: image.into(),
            deployed_at: Utc::now(),
        }
    }
}

/// GCP Cloud Run Job deployment output.
///
/// WHY: Stores the minimal information needed to identify and destroy a Cloud Run Job.
///
/// WHAT: Contains resource name, job name, region, and project ID.
///
/// HOW: Returned by `CloudRunJob::deploy()` and stored in state store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudRunJobDeployment {
    /// Full resource path (e.g., `projects/my-project/locations/us-central1/jobs/my-job`).
    pub resource_name: String,
    /// Job name.
    pub job_name: String,
    /// GCP region (e.g., `us-central1`).
    pub region: String,
    /// GCP project ID.
    pub project_id: String,
    /// Timestamp of the deployment.
    pub deployed_at: DateTime<Utc>,
}

impl CloudRunJobDeployment {
    /// Create a new CloudRunJobDeployment.
    ///
    /// # Arguments
    ///
    /// * `resource_name` - Full resource path
    /// * `job_name` - Job name
    /// * `region` - GCP region
    /// * `project_id` - GCP project ID
    #[must_use]
    pub fn new(
        resource_name: impl Into<String>,
        job_name: impl Into<String>,
        region: impl Into<String>,
        project_id: impl Into<String>,
    ) -> Self {
        Self {
            resource_name: resource_name.into(),
            job_name: job_name.into(),
            region: region.into(),
            project_id: project_id.into(),
            deployed_at: Utc::now(),
        }
    }
}

/// Generic deployment output when you just need basic info.
///
/// WHY: Some deployments don't fit the standard types above.
///
/// WHAT: Flexible type with metadata stored as JSON.
///
/// HOW: Can be used for custom deployments or as a fallback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentOutput {
    /// Unique identifier for the deployment.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Optional URL.
    pub url: Option<String>,
    /// Additional metadata.
    pub metadata: serde_json::Value,
    /// Timestamp of the deployment.
    pub deployed_at: DateTime<Utc>,
}

impl DeploymentOutput {
    /// Create a new DeploymentOutput.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier
    /// * `name` - Human-readable name
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            url: None,
            metadata: serde_json::Value::Null,
            deployed_at: Utc::now(),
        }
    }

    /// Set the URL.
    #[must_use]
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Set the metadata.
    #[must_use]
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}
