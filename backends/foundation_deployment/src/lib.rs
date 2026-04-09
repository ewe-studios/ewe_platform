//! Foundation Deployment - Multi-cloud deployment providers.
//!
//! WHY: Provides deployment providers for various cloud platforms (Cloudflare, GCP, AWS).
//!
//! WHAT: Implements the `DeploymentProvider` trait for each supported cloud platform,
//! handling deployment, rollback, and verification.
//!
//! HOW: Uses `SimpleHttpClient` for API-first deployments, with optional CLI fallback.

pub mod config;
pub mod core;
pub mod engine;
pub mod error;
pub mod json_schema;
pub mod provider_client;
pub mod providers;
pub mod resource_info;
