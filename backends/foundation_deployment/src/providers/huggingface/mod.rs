//! Hugging Face Hub API provider.
//!
//! This module provides a Rust implementation of the Hugging Face Hub API
//! using only `valtron` and `simple_http` - no tokio, reqwest, or async-trait.
//!
//! # Module Organization
//!
//! - [`client`] - HFClient struct and core API operations (whoami, list_*, create_repo, etc.)
//! - [`repository`] - HFRepository struct and repository operations (info, download, upload, etc.)
//! - [`types`] - Type definitions for all API requests and responses
//! - [`constants`] - API endpoints and configuration constants
//! - [`error`] - Error types and Result alias
//!
//! # Usage
//!
//! ```rust,no_run
//! use foundation_deployment::providers::huggingface::{HFClient, client, repository, types};
//!
//! // Initialize client
//! let client = HFClient::builder().token("hf_...").build()?;
//!
//! // Client operations
//! let user = client::whoami(&client)?;
//! let models = client::list_models(&client, &types::ListModelsParams::default())?;
//!
//! // Repository operations
//! let repo = client.model("owner", "repo-name");
//! let info = repository::repo_info(&repo, &types::RepoInfoParams::default())?;
//! ```

pub mod client;
pub mod constants;
pub mod error;
pub mod repository;
pub mod types;

pub use client::{HFClient, HFClientBuilder};
pub use repository::{HFRepository, RepositoryArgs};
pub use error::{HuggingFaceError, Result};
pub use types::{
    AddSource, CommitInfo, CommitOperation, CommitAuthor, DatasetInfo, DiffEntry,
    GitCommitInfo, ListDatasetsParams, ListModelsParams, ListSpacesParams, ModelInfo,
    RepoDownloadFileParams, RepoInfo, RepoListTreeParams, RepoSibling, RepoTreeEntry, RepoType, RepoUrl,
    RepoUploadFileParams, RepoCreateCommitParams, RepoDeleteFileParams, RepoInfoParams,
    SpaceInfo, User,
};
