//! Hugging Face Hub API provider.
//!
//! This module provides a Rust implementation of the Hugging Face Hub API
//! using only `valtron` and `simple_http` - no tokio, reqwest, or async-trait.
//!
//! # Module Organization
//!
//! - [`client`] - `HFClient` struct and core API operations (whoami, list_*, `create_repo`, etc.)
//! - [`repository`] - `HFRepository` struct and repository operations (info, download, upload, etc.)
//! - [`types`] - Type definitions for all API requests and responses
//! - [`constants`] - API endpoints and configuration constants
//! - [`error`] - Error types and Result alias
//!
//! # Usage
//!
//! ```rust,no_run
//! use foundation_core::valtron::block_on_future;
//! use foundation_deployment_huggingface::{HFClient, client, repository, types};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Initialize client
//! let client = HFClient::builder().token("hf_...").build()?;
//!
//! // Client operations are async; wrap them in a blocking future at the
//! // sync boundary.
//! block_on_future({ let c = client.clone(); async move { client::whoami(&c).await } })?;
//! block_on_future({ let c = client.clone(); async move { client::list_models(&c, &types::ListModelsParams::default()).await } })?;
//!
//! // Repository operations
//! let repo = client.model("owner", "repo-name");
//! block_on_future({ let r = repo.clone(); async move { repository::repo_info(&r, &types::RepoInfoParams::default()).await } })?;
//! # Ok(())
//! # }
//! ```

pub mod client;
pub mod constants;
pub mod error;
pub mod repository;
pub mod types;

pub use client::{HFClient, HFClientBuilder};
pub use error::{HuggingFaceError, Result};
pub use repository::{HFRepository, RepositoryArgs};
pub use types::*;
