//! Hugging Face Hub API provider.
//!
//! This module provides a Rust implementation of the Hugging Face Hub API
//! using only `valtron` and `simple_http` - no tokio, reqwest, or async-trait.

pub mod client;
pub mod constants;
pub mod error;
pub mod repository;
pub mod types;

pub use client::{HFClient, HFClientBuilder};
pub use error::{HuggingFaceError, Result};
pub use repository::HFRepository;
pub use types::{
    AddSource, CommitInfo, CommitOperation, CommitAuthor, DatasetInfo, DiffEntry,
    GitCommitInfo, ListDatasetsParams, ListModelsParams, ListSpacesParams, ModelInfo,
    RepoDownloadFileParams, RepoInfo, RepoListTreeParams, RepoSibling, RepoTreeEntry, RepoType, RepoUrl,
    RepoUploadFileParams, RepoCreateCommitParams, RepoDeleteFileParams, RepoInfoParams,
    SpaceInfo, User,
};
