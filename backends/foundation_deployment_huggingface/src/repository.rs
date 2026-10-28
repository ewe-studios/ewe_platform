//! Repository handle and operations for Hugging Face Hub.
//!
//! This module provides the [`HFRepository`] struct and repository-specific operations.
//! All functions take a `&HFRepository` as the first parameter.

#![allow(clippy::too_many_lines)]

use crate::client::{is_success_status, HFClient};
use crate::error::{HuggingFaceError, Result};
use crate::types::{
    AddSource, CommitInfo, CommitOperation, RepoCreateCommitParams, RepoDeleteFileParams,
    RepoDownloadFileParams, RepoInfo, RepoInfoParams, RepoListTreeParams, RepoTreeEntry, RepoType,
    RepoUploadFileParams,
};
use foundation_core::valtron::Stream;
use foundation_netio::shared::client::body_reader::collect_bytes_into;
use foundation_netio::shared::http::{SimpleHeader, SimpleMethod};

use foundation_macros::JsonHash;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;

/// Handle for a single repository.
#[derive(Clone)]
pub struct HFRepository {
    inner: Arc<HFRepositoryInner>,
}

struct HFRepositoryInner {
    client: HFClient,
    owner: String,
    name: String,
    repo_type: RepoType,
}

/// Arguments for creating a repository handle.
#[derive(Debug, Clone, Serialize, JsonHash)]
pub struct RepositoryArgs {
    pub owner: String,
    pub name: String,
    pub repo_type: RepoType,
    pub default_revision: Option<String>,
}

impl HFRepository {
    /// Create a new repository handle.
    #[must_use]
    pub fn new(client: HFClient, owner: String, name: String, repo_type: RepoType) -> Self {
        Self {
            inner: Arc::new(HFRepositoryInner {
                client,
                owner,
                name,
                repo_type,
            }),
        }
    }

    /// Create a new repository handle with arguments.
    #[must_use]
    pub fn with_args(client: HFClient, args: RepositoryArgs) -> Self {
        let RepositoryArgs {
            owner,
            name,
            repo_type,
            default_revision: _,
        } = args;
        Self {
            inner: Arc::new(HFRepositoryInner {
                client,
                owner,
                name,
                repo_type,
            }),
        }
    }

    /// Get the repository path (owner/name).
    #[must_use]
    pub fn repo_path(&self) -> String {
        format!("{}/{}", self.inner.owner, self.inner.name)
    }

    /// Get the repository type.
    #[must_use]
    pub fn repo_type(&self) -> RepoType {
        self.inner.repo_type
    }
}

// ============================================================================
// Repository API Functions
// ============================================================================
// Operations that work with an HFRepository instance.

/// Get repository info.
///
/// # Errors
///
/// Returns `HuggingFaceError::Http` if the Hub responds with a non-2xx
/// status (e.g., 404 if the repo or revision does not exist),
/// `HuggingFaceError::Json` if the response cannot be deserialized into
/// `RepoInfo`, or `HuggingFaceError::Backend` for a transport failure.
pub fn repo_info(repo: &HFRepository, params: &RepoInfoParams) -> Result<RepoInfo> {
    let url = match repo.inner.repo_type {
        RepoType::Dataset => format!(
            "{}/api/datasets/{}",
            repo.inner.client.endpoint(),
            repo.repo_path()
        ),
        RepoType::Space => format!(
            "{}/api/spaces/{}",
            repo.inner.client.endpoint(),
            repo.repo_path()
        ),
        _ => format!(
            "{}/api/models/{}",
            repo.inner.client.endpoint(),
            repo.repo_path()
        ),
    };

    let url = if let Some(ref revision) = params.revision {
        format!("{url}/revision/{revision}")
    } else {
        url
    };

    let client = &repo.inner.client;
    let request = client.request(SimpleMethod::GET, &url, true)?.build();
    client.send_json(request, &url)
}

/// Check if repository exists.
///
/// # Errors
///
/// Returns `HuggingFaceError::Backend` if the HTTP request cannot be built or
/// the request fails in transport (the Hub could not be asked). Any non-2xx
/// answer from the Hub is `Ok(false)`.
pub fn repo_exists(repo: &HFRepository) -> Result<bool> {
    let url = match repo.inner.repo_type {
        RepoType::Dataset => format!(
            "{}/api/datasets/{}",
            repo.inner.client.endpoint(),
            repo.repo_path()
        ),
        RepoType::Space => format!(
            "{}/api/spaces/{}",
            repo.inner.client.endpoint(),
            repo.repo_path()
        ),
        _ => format!(
            "{}/api/models/{}",
            repo.inner.client.endpoint(),
            repo.repo_path()
        ),
    };

    // A 2xx answer means it exists; any other status means it does not. A
    // transport failure is an error, not "does not exist". The body is
    // dropped unread.
    let client = &repo.inner.client;
    let request = client.request(SimpleMethod::GET, &url, true)?.build();
    let (status, _body) = client.send_raw(request)?;
    Ok(is_success_status(&status))
}

/// Check if revision exists.
///
/// # Errors
///
/// Returns `HuggingFaceError::Backend` if the HTTP request cannot be built or
/// the request fails in transport. Any non-2xx answer (missing revision) is
/// `Ok(false)`.
pub fn repo_revision_exists(repo: &HFRepository, revision: &str) -> Result<bool> {
    let url = match repo.inner.repo_type {
        RepoType::Dataset => format!(
            "{}/api/datasets/{}/revision/{}",
            repo.inner.client.endpoint(),
            repo.repo_path(),
            revision
        ),
        RepoType::Space => format!(
            "{}/api/spaces/{}/revision/{}",
            repo.inner.client.endpoint(),
            repo.repo_path(),
            revision
        ),
        _ => format!(
            "{}/api/models/{}/revision/{}",
            repo.inner.client.endpoint(),
            repo.repo_path(),
            revision
        ),
    };

    // A 2xx answer means it exists; any other status means it does not. A
    // transport failure is an error, not "does not exist". The body is
    // dropped unread.
    let client = &repo.inner.client;
    let request = client.request(SimpleMethod::GET, &url, true)?.build();
    let (status, _body) = client.send_raw(request)?;
    Ok(is_success_status(&status))
}

/// List tree entries.
///
/// # Errors
///
/// Returns `HuggingFaceError::Backend` if the HTTP request cannot be built. Per-page
/// errors (HTTP failures, JSON parse failures) surface as `Err` items inside
/// the returned stream.
pub fn repo_list_tree(
    repo: &HFRepository,
    params: &RepoListTreeParams,
) -> Result<impl Iterator<Item = Stream<Result<RepoTreeEntry>, ()>> + Send + 'static> {
    let base_path = match repo.inner.repo_type {
        RepoType::Dataset => "datasets",
        RepoType::Space => "spaces",
        _ => "models",
    };
    let revision = params.revision.as_deref().unwrap_or("main");
    let url = format!(
        "{}/api/{}/{}/tree/{}",
        repo.inner.client.endpoint(),
        base_path,
        repo.repo_path(),
        revision
    );

    let client = &repo.inner.client;
    let request = client.request(SimpleMethod::GET, &url, true)?.build();
    let page = client.send_json::<Vec<RepoTreeEntry>>(request, &url);
    let items: Vec<Stream<Result<RepoTreeEntry>, ()>> = match page {
        Ok(entries) => entries.into_iter().map(|e| Stream::Next(Ok(e))).collect(),
        Err(e) => vec![Stream::Next(Err(e))],
    };
    Ok(items.into_iter())
}

/// Download a file, handling `HuggingFace`'s CDN redirect.
///
/// `HuggingFace` returns a 302 redirect to their CDN for file downloads; the
/// client follows it and keeps the auth header. A non-2xx answer is an error
/// (its body, up to `max_error_body`, is the message) and nothing is written.
///
/// # Errors
///
/// Returns an error if the HTTP request fails, the redirect URL is malformed,
/// or the file cannot be written to the destination directory.
pub fn repo_download_file(repo: &HFRepository, params: &RepoDownloadFileParams) -> Result<PathBuf> {
    tracing::debug!("Starting download of file from huggingface={:?}", &params);

    let revision = params.revision.as_deref().unwrap_or("main");
    let url = repo.inner.client.download_url(
        Some(repo.inner.repo_type),
        &repo.repo_path(),
        revision,
        &params.filename,
    );

    // Build destination path: directory/filename
    let destination = params.directory.join(&params.filename);

    // The client follows the Hub's redirect to its CDN, keeping the auth
    // header (see `HFClientBuilder::build`).
    let client = &repo.inner.client;
    let request = client.request(SimpleMethod::GET, &url, true)?.build();

    tracing::debug!("Call endpoint for huggingface={:?}", &params);

    // Status first: an error answer (404 for a missing file, 401/403 for a
    // gated model) must never be written to disk as the file.
    let (status, body) = client.send_raw(request)?;
    tracing::debug!("Received response with status={}", &status);
    if !is_success_status(&status) {
        return Err(client.http_error(&status, &url, body));
    }

    // Stream the body to a temporary `.part` file and only rename it into place
    // once the full download succeeds. This prevents a failed/partial download
    // from leaving a truncated (e.g. 0-byte) file that later looks like a valid
    // cached artifact.
    let mut temp_os = destination.clone().into_os_string();
    temp_os.push(".part");
    let temp_destination = PathBuf::from(temp_os);

    let mut file = std::fs::File::create(&temp_destination).map_err(HuggingFaceError::Io)?;
    if let Err(e) = collect_bytes_into(body, &mut file) {
        drop(file);
        let _ = std::fs::remove_file(&temp_destination);
        return Err(HuggingFaceError::Backend(e.to_string()));
    }
    drop(file);

    std::fs::rename(&temp_destination, &destination).map_err(HuggingFaceError::Io)?;

    Ok(destination)
}

/// Upload a file.
///
/// # Errors
///
/// Returns an error if the upload request fails or the commit cannot be created.
pub fn repo_upload_file(repo: &HFRepository, params: &RepoUploadFileParams) -> Result<CommitInfo> {
    let commit_params = RepoCreateCommitParams {
        operations: vec![CommitOperation::Add {
            path_in_repo: params.path_in_repo.clone(),
            source: params.source.clone(),
        }],
        commit_message: params
            .commit_message
            .clone()
            .unwrap_or_else(|| format!("Upload {}", params.path_in_repo)),
        commit_description: None,
        revision: params.revision.clone(),
        create_pr: None,
    };
    repo_create_commit(repo, &commit_params)
}

/// Delete a file.
///
/// # Errors
///
/// Returns an error if the delete request fails or the commit cannot be created.
pub fn repo_delete_file(repo: &HFRepository, params: &RepoDeleteFileParams) -> Result<CommitInfo> {
    let commit_params = RepoCreateCommitParams {
        operations: vec![CommitOperation::Delete {
            path_in_repo: params.path_in_repo.clone(),
        }],
        commit_message: params
            .commit_message
            .clone()
            .unwrap_or_else(|| format!("Delete {}", params.path_in_repo)),
        commit_description: None,
        revision: params.revision.clone(),
        create_pr: None,
    };
    repo_create_commit(repo, &commit_params)
}

/// Create a commit with multiple operations.
///
/// # Errors
///
/// Returns `HuggingFaceError::Io` if a local source file cannot be read,
/// `HuggingFaceError::Json` if the embedded summary cannot be serialised,
/// `HuggingFaceError::Http` if the Hub responds with a non-2xx status, or
/// `HuggingFaceError::Backend` for transport failures.
pub fn repo_create_commit(
    repo: &HFRepository,
    params: &RepoCreateCommitParams,
) -> Result<CommitInfo> {
    use std::io::Write as _;

    let base_path = match repo.inner.repo_type {
        RepoType::Dataset => "datasets",
        RepoType::Space => "spaces",
        _ => "models",
    };
    let revision = params.revision.as_deref().unwrap_or("main");
    let url = format!(
        "{}/api/{}/{}/commit/{}",
        repo.inner.client.endpoint(),
        base_path,
        repo.repo_path(),
        revision
    );

    // Build multipart body. Bytes, not a `String`: an added file can be
    // binary (weights, images), so it must not pass through UTF-8.
    let boundary = "----RustBoundary".to_string();
    let mut body: Vec<u8> = Vec::new();

    let _ = write!(body, "--{boundary}\r\n");
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"summary\"\r\n\r\n");
    body.extend_from_slice(
        serde_json::to_string(&serde_json::json!({
            "type": "commit",
            "summary": params.commit_message,
            "description": params.commit_description,
            "hub": "huggingface.co",
        }))?
        .as_bytes(),
    );
    body.extend_from_slice(b"\r\n");

    for op in &params.operations {
        match op {
            CommitOperation::Add {
                path_in_repo,
                source,
            } => {
                let _ = write!(body, "--{boundary}\r\n");
                let _ = write!(
                    body,
                    "Content-Disposition: form-data; name=\"files\"; filename=\"{path_in_repo}\"\r\n"
                );
                body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
                match source {
                    AddSource::Bytes(data) => body.extend_from_slice(data),
                    AddSource::File(path) => {
                        let content = std::fs::read(path).map_err(HuggingFaceError::Io)?;
                        body.extend_from_slice(&content);
                    }
                }
                body.extend_from_slice(b"\r\n");
            }
            CommitOperation::Delete { path_in_repo } => {
                let _ = write!(body, "--{boundary}\r\n");
                body.extend_from_slice(
                    b"Content-Disposition: form-data; name=\"deletedFiles\"\r\n\r\n",
                );
                body.extend_from_slice(path_in_repo.as_bytes());
                body.extend_from_slice(b"\r\n");
            }
        }
    }

    let _ = write!(body, "--{boundary}--\r\n");

    let client = &repo.inner.client;
    let content_type = format!("multipart/form-data; boundary={boundary}");
    let request = client
        .request(SimpleMethod::POST, &url, true)?
        .header(SimpleHeader::CONTENT_TYPE, content_type)
        .body_bytes(body)
        .build();
    client.send_json(request, &url)
}
