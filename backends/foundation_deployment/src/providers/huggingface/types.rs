//! Type definitions for Hugging Face Hub API.

use foundation_macros::JsonHash;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Type of repository on the Hub.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonHash)]
#[serde(rename_all = "lowercase")]
pub enum RepoType {
    Model,
    Dataset,
    Space,
    Kernel,
}

impl fmt::Display for RepoType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RepoType::Model => write!(f, "model"),
            RepoType::Dataset => write!(f, "dataset"),
            RepoType::Space => write!(f, "space"),
            RepoType::Kernel => write!(f, "kernel"),
        }
    }
}

impl Default for RepoType {
    fn default() -> Self {
        RepoType::Model
    }
}

/// Blob LFS information.
#[derive(Debug, Clone, Serialize, Deserialize, JsonHash)]
pub struct BlobLfsInfo {
    pub size: Option<u64>,
    pub sha256: Option<String>,
    pub pointer_size: Option<u64>,
}

/// Last commit information.
#[derive(Debug, Clone, Serialize, Deserialize, JsonHash)]
#[serde(rename_all = "camelCase")]
pub struct LastCommitInfo {
    pub id: Option<String>,
    pub title: Option<String>,
    pub date: Option<String>,
}

/// Repository sibling (file in repository).
#[derive(Debug, Clone, Serialize, Deserialize, JsonHash)]
pub struct RepoSibling {
    pub rfilename: String,
    pub size: Option<u64>,
    pub lfs: Option<BlobLfsInfo>,
}

/// Tagged union for tree entries returned by list_repo_tree.
#[derive(Debug, Clone, Deserialize, JsonHash)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RepoTreeEntry {
    File {
        oid: String,
        size: u64,
        path: String,
        lfs: Option<BlobLfsInfo>,
        #[serde(default, rename = "lastCommit")]
        last_commit: Option<LastCommitInfo>,
    },
    Directory {
        oid: String,
        path: String,
    },
}

/// Model information returned by GET /api/models/{id}.
#[derive(Debug, Clone, Serialize, Deserialize, JsonHash)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    #[serde(rename = "_id")]
    pub mongo_id: Option<String>,
    pub model_id: Option<String>,
    pub author: Option<String>,
    pub sha: Option<String>,
    pub private: Option<bool>,
    pub gated: Option<serde_json::Value>,
    pub disabled: Option<bool>,
    pub downloads: Option<u64>,
    pub downloads_all_time: Option<u64>,
    pub likes: Option<u64>,
    pub tags: Option<Vec<String>>,
    #[serde(rename = "pipeline_tag")]
    pub pipeline_tag: Option<String>,
    #[serde(rename = "library_name")]
    pub library_name: Option<String>,
    pub created_at: Option<String>,
    pub last_modified: Option<String>,
    pub siblings: Option<Vec<RepoSibling>>,
    pub card_data: Option<serde_json::Value>,
    pub config: Option<serde_json::Value>,
    pub trending_score: Option<f64>,
    pub gguf: Option<serde_json::Value>,
    pub spaces: Option<Vec<String>>,
    pub used_storage: Option<u64>,
    pub widget_data: Option<serde_json::Value>,
}

/// Dataset information.
#[derive(Debug, Clone, Serialize, Deserialize, JsonHash)]
#[serde(rename_all = "camelCase")]
pub struct DatasetInfo {
    pub id: String,
    #[serde(rename = "_id")]
    pub mongo_id: Option<String>,
    pub author: Option<String>,
    pub sha: Option<String>,
    pub private: Option<bool>,
    pub gated: Option<serde_json::Value>,
    pub disabled: Option<bool>,
    pub downloads: Option<u64>,
    pub downloads_all_time: Option<u64>,
    pub likes: Option<u64>,
    pub tags: Option<Vec<String>>,
    pub created_at: Option<String>,
    pub last_modified: Option<String>,
    pub siblings: Option<Vec<RepoSibling>>,
    pub card_data: Option<serde_json::Value>,
    pub trending_score: Option<f64>,
    pub description: Option<String>,
    pub used_storage: Option<u64>,
}

/// Space information.
#[derive(Debug, Clone, Serialize, Deserialize, JsonHash)]
#[serde(rename_all = "camelCase")]
pub struct SpaceInfo {
    pub id: String,
    #[serde(rename = "_id")]
    pub mongo_id: Option<String>,
    pub author: Option<String>,
    pub sha: Option<String>,
    pub private: Option<bool>,
    pub gated: Option<serde_json::Value>,
    pub disabled: Option<bool>,
    pub likes: Option<u64>,
    pub tags: Option<Vec<String>>,
    pub created_at: Option<String>,
    pub last_modified: Option<String>,
    pub siblings: Option<Vec<RepoSibling>>,
    pub card_data: Option<serde_json::Value>,
    pub sdk: Option<String>,
    pub trending_score: Option<f64>,
    pub host: Option<String>,
    pub subdomain: Option<String>,
    pub runtime: Option<serde_json::Value>,
    pub used_storage: Option<u64>,
}

/// Union type for repository info responses.
#[derive(Debug, Clone, JsonHash)]
pub enum RepoInfo {
    Model(ModelInfo),
    Dataset(DatasetInfo),
    Space(SpaceInfo),
}

impl RepoInfo {
    /// Get the repository type.
    pub fn repo_type(&self) -> RepoType {
        match self {
            RepoInfo::Model(_) => RepoType::Model,
            RepoInfo::Dataset(_) => RepoType::Dataset,
            RepoInfo::Space(_) => RepoType::Space,
        }
    }
}

/// User information.
#[derive(Debug, Clone, Deserialize, JsonHash)]
pub struct User {
    pub username: String,
    pub fullname: Option<String>,
    pub avatar_url: Option<String>,
    #[serde(rename = "type")]
    pub user_type: Option<String>,
    pub is_pro: Option<bool>,
    pub email: Option<String>,
    pub orgs: Option<Vec<OrgMembership>>,
}

/// Organization membership.
#[derive(Debug, Clone, Deserialize, JsonHash)]
pub struct OrgMembership {
    pub name: String,
    pub fullname: Option<String>,
    pub avatar_url: Option<String>,
}

/// Organization information.
#[derive(Debug, Clone, Deserialize, JsonHash)]
pub struct Organization {
    pub name: String,
    pub fullname: Option<String>,
    pub avatar_url: Option<String>,
    #[serde(rename = "type")]
    pub org_type: Option<String>,
}

/// Commit author information.
#[derive(Debug, Clone, Deserialize, JsonHash)]
pub struct CommitAuthor {
    pub user: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
}

/// Git commit information.
#[derive(Debug, Clone, Deserialize, JsonHash)]
pub struct GitCommitInfo {
    pub id: String,
    pub authors: Vec<CommitAuthor>,
    pub date: Option<String>,
    pub title: String,
    pub message: String,
    #[serde(default)]
    pub parents: Vec<String>,
}

/// Commit information returned by commit operations.
#[derive(Debug, Clone, Deserialize, JsonHash)]
#[serde(rename_all = "camelCase")]
pub struct CommitInfo {
    pub commit_url: Option<String>,
    pub commit_message: Option<String>,
    pub commit_description: Option<String>,
    pub commit_oid: Option<String>,
    pub pr_url: Option<String>,
    pub pr_num: Option<u64>,
}

/// A single entry in a commit diff.
#[derive(Debug, Clone, Deserialize, JsonHash)]
#[serde(rename_all = "camelCase")]
pub struct DiffEntry {
    pub path: Option<String>,
    pub old_path: Option<String>,
    pub status: Option<String>,
}

/// Source of content for an add operation.
#[derive(Debug, Clone, JsonHash)]
pub enum AddSource {
    File(PathBuf),
    Bytes(Vec<u8>),
}

/// A file operation in a commit.
#[derive(Debug, Clone, JsonHash)]
pub enum CommitOperation {
    /// Upload a file (from path or bytes).
    Add {
        path_in_repo: String,
        source: AddSource,
    },
    /// Delete a file or folder.
    Delete { path_in_repo: String },
}

/// URL returned by create_repo/move_repo.
#[derive(Debug, Clone, Deserialize, JsonHash)]
pub struct RepoUrl {
    pub url: String,
}

/// Parameters for listing models.
#[derive(Debug, Clone, Default)]
pub struct ListModelsParams {
    pub search: Option<String>,
    pub author: Option<String>,
    pub filter: Option<String>,
    pub sort: Option<String>,
    pub pipeline_tag: Option<String>,
    pub full: Option<bool>,
    pub card_data: Option<bool>,
    pub fetch_config: Option<bool>,
    pub limit: Option<usize>,
}

/// Parameters for listing datasets.
#[derive(Debug, Clone, Default)]
pub struct ListDatasetsParams {
    pub search: Option<String>,
    pub author: Option<String>,
    pub filter: Option<String>,
    pub sort: Option<String>,
    pub full: Option<bool>,
    pub limit: Option<usize>,
}

/// Parameters for listing spaces.
#[derive(Debug, Clone, Default)]
pub struct ListSpacesParams {
    pub search: Option<String>,
    pub author: Option<String>,
    pub filter: Option<String>,
    pub sort: Option<String>,
    pub full: Option<bool>,
    pub limit: Option<usize>,
}

/// Parameters for creating a repository.
#[derive(Debug, Clone, Default)]
pub struct CreateRepoParams {
    pub repo_id: String,
    pub repo_type: Option<RepoType>,
    pub private: Option<bool>,
    pub exist_ok: bool,
    pub space_sdk: Option<String>,
}

/// Parameters for deleting a repository.
#[derive(Debug, Clone, Default)]
pub struct DeleteRepoParams {
    pub repo_id: String,
    pub repo_type: Option<RepoType>,
    pub missing_ok: bool,
}

/// Parameters for moving a repository.
#[derive(Debug, Clone, Default)]
pub struct MoveRepoParams {
    pub from_id: String,
    pub to_id: String,
    pub repo_type: Option<RepoType>,
}

/// Parameters for repository info.
#[derive(Debug, Clone, Default)]
pub struct RepoInfoParams {
    pub revision: Option<String>,
}

/// Parameters for repository tree listing.
#[derive(Debug, Clone, Default)]
pub struct RepoListTreeParams {
    pub revision: Option<String>,
    pub recursive: Option<bool>,
    pub limit: Option<usize>,
}

/// Parameters for file download.
#[derive(Debug, Clone, Default)]
pub struct RepoDownloadFileParams {
    pub filename: String,
    pub revision: Option<String>,
    pub destination: Option<PathBuf>,
}

/// Parameters for file upload.
#[derive(Debug, Clone, Default)]
pub struct RepoUploadFileParams {
    pub source: AddSource,
    pub path_in_repo: String,
    pub revision: Option<String>,
    pub commit_message: Option<String>,
}

/// Parameters for creating a commit.
#[derive(Debug, Clone, Default)]
pub struct RepoCreateCommitParams {
    pub operations: Vec<CommitOperation>,
    pub commit_message: String,
    pub commit_description: Option<String>,
    pub revision: Option<String>,
    pub create_pr: Option<bool>,
}

/// Parameters for deleting a file.
#[derive(Debug, Clone, Default)]
pub struct RepoDeleteFileParams {
    pub path_in_repo: String,
    pub revision: Option<String>,
    pub commit_message: Option<String>,
}

use std::fmt;
