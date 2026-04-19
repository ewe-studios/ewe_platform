//! Provider-specific deployment implementations.
//!
//! Each provider gets its own directory containing:
//! - `provider.rs` — `DeploymentProvider` trait implementation
//! - `fetch.rs` — `OpenAPI` spec fetcher
//! - `resources.rs` — (future) auto-generated resource types from the spec

pub mod aws;
pub mod cloudflare;
pub mod common;
pub mod fly_io;
pub mod gcp;
pub mod huggingface;
pub mod mongodb_atlas;
pub mod neon;
pub mod openapi;
pub mod planetscale;
pub mod prisma_postgres;
pub mod standard;
pub mod stripe;
pub mod supabase;
