//! Ewe Deployables - Ready-to-use `Deployable` implementations.
//!
//! WHY: Users need pre-built deployables for common cloud resources without writing
//!      the deployment logic themselves.
//!
//! WHAT: Ready-to-use `Deployable` implementations for Cloudflare Workers,
//!       GCP Cloud Run services, and GCP Cloud Run Jobs.
//!
//! HOW: Each deployable implements the `Deployable` trait from `foundation_deployment`.
//!      Users create instances and call `deploy()` or `destroy()` with a `ProviderClient`.
//!
//! # Examples
//!
//! ```rust,no_run
//! use ewe_deployables::cloudflare::CloudflareWorker;
//! use ewe_deployables::gcp::CloudRunService;
//! use foundation_deployment::provider_client::ProviderClient;
//! use foundation_db::state::FileStateStore;
//! use foundation_core::wire::simple_http::client::SimpleHttpClient;
//!
//! // Deploy a Cloudflare Worker
//! let worker: CloudflareWorker = CloudflareWorker::new("my-worker", "./worker.js", "account-id");
//! // let client = ProviderClient::new("my-project", "dev", state_store, http_client);
//! // let stream = worker.deploy(client)?;
//!
//! // Deploy a GCP Cloud Run service
//! let service: CloudRunService = CloudRunService::new("my-service", "gcr.io/project/image:latest", "us-central1", "project-id");
//! // let stream = service.deploy(client)?;
//! ```
//!
//! # Modules
//!
//! - `cloudflare` - Cloudflare Workers deployables
//! - `gcp` - GCP Cloud Run and Cloud Run Jobs deployables
//! - `common` - Common types re-exported from `foundation_deployment`

pub mod cloudflare;
pub mod common;
pub mod gcp;

// Re-export common types
pub use common::types::*;

// Re-export trait and related types
pub use foundation_deployment::traits::{Deployable, Deploying};
