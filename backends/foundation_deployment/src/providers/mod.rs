//! Provider-specific deployment implementations.
//!
//! Each provider gets its own directory containing:
//! - `provider.rs` — `DeploymentProvider` trait implementation
//! - `fetch.rs` — `OpenAPI` spec fetcher
//! - `resources.rs` — (future) auto-generated resource types from the spec
//!
//! Note: Most providers have been moved to standalone `foundation_deployment_*` crates:
//! - `foundation_deployment_huggingface`
//! - `foundation_deployment_cloudflare`
//! - `foundation_deployment_flyio`
//! - `foundation_deployment_gcp`
//! - `foundation_deployment_mongoatlas`
//! - `foundation_deployment_neon`
//! - `foundation_deployment_planetscale`
//! - `foundation_deployment_prisma`
//! - `foundation_deployment_stripe`
//! - `foundation_deployment_supabase`

pub mod common;
pub mod openapi;
pub mod standard;
