//! GCP deployables.

mod cloud_run;

pub use cloud_run::{CloudRunJob, CloudRunJobError, CloudRunService, CloudRunServiceError};
