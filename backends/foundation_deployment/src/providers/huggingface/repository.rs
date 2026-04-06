//! Repository handle and operations for Hugging Face Hub.
//!
//! This module provides the [`HFRepository`] struct and repository-specific operations.
//! All functions take a `&HFRepository` as the first parameter.

use crate::providers::huggingface::client::{is_success_status, status_code, HFClient};
use crate::providers::huggingface::constants;
use crate::providers::huggingface::error::{HuggingFaceError, Result};
use crate::providers::huggingface::types::{
    AddSource, CommitInfo, CommitOperation, RepoCreateCommitParams, RepoDeleteFileParams,
    RepoDownloadFileParams, RepoInfo, RepoInfoParams, RepoListTreeParams, RepoTreeEntry, RepoType,
    RepoUploadFileParams,
};
use foundation_core::valtron::{collect_one, execute, Stream, StreamIteratorExt, TaskIteratorExt};
use foundation_core::wire::simple_http::client::body_reader;
use foundation_core::wire::simple_http::client::RequestIntro;
use foundation_core::wire::simple_http::SimpleHeader;
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
    pub fn new(
        client: HFClient,
        owner: String,
        name: String,
        repo_type: RepoType,
    ) -> Self {
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
    pub fn repo_path(&self) -> String {
        format!("{}/{}", self.inner.owner, self.inner.name)
    }

    /// Get the repository type.
    pub fn repo_type(&self) -> RepoType {
        self.inner.repo_type
    }
}

// ============================================================================
// Repository API Functions
// ============================================================================
// Operations that work with an HFRepository instance.

/// Get repository info.
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
        format!("{}/revision/{}", url, revision)
    } else {
        url
    };

    let http_client = repo.inner.client.simple_http();

    let builder = http_client
        .get(&url)
        .map_err(|e| HuggingFaceError::Backend(e.to_string()))?
        .header(SimpleHeader::USER_AGENT, constants::HF_USER_AGENT);

    let builder = {
        if let Some(ref token) = repo.inner.client.token() {
            if !HFClient::is_implicit_token_disabled() {
                builder.header(SimpleHeader::AUTHORIZATION, format!("Bearer {}", token))
            } else {
                builder
            }
        } else {
            builder
        }
    };

    let task = builder
        .build_send_request()
        .map_err(|e| HuggingFaceError::Backend(e.to_string()))?
        .map_ready(move |intro| match intro {
            RequestIntro::Success { stream, intro, .. } => {
                let status = &intro.0;
                if !is_success_status(status) {
                    return Err(HuggingFaceError::Http {
                        status: status_code(status),
                        url: url.clone(),
                        body: format!("HTTP {}", status_code(status)),
                    });
                }
                let body = body_reader::collect_string(stream);
                let info: RepoInfo = serde_json::from_str(&body)
                    .map_err(HuggingFaceError::Json)?;
                Ok(info)
            }
            RequestIntro::Failed(e) => Err(HuggingFaceError::Backend(format!(
                "Request failed: {}",
                e
            ))),
        })
        .map_pending(|_| ());

    let stream = execute(task, None)
        .map_err(|e| HuggingFaceError::Valtron(e.to_string()))?;

    collect_one(stream)
        .ok_or_else(|| HuggingFaceError::Backend("No result from repo_info".into()))?
}

/// Check if repository exists.
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

    let http_client = repo.inner.client.simple_http();

    let builder = http_client
        .get(&url)
        .map_err(|e| HuggingFaceError::Backend(e.to_string()))?
        .header(SimpleHeader::USER_AGENT, constants::HF_USER_AGENT);

    let builder = {
        if let Some(ref token) = repo.inner.client.token() {
            if !HFClient::is_implicit_token_disabled() {
                builder.header(SimpleHeader::AUTHORIZATION, format!("Bearer {}", token))
            } else {
                builder
            }
        } else {
            builder
        }
    };

    let task = builder
        .build_send_request()
        .map_err(|e| HuggingFaceError::Backend(e.to_string()))?
        .map_ready(move |intro| match intro {
            RequestIntro::Success { intro, .. } => {
                let status = &intro.0;
                Ok(is_success_status(status))
            }
            RequestIntro::Failed(_) => Ok(false),
        })
        .map_pending(|_| ());

    let stream = execute(task, None)
        .map_err(|e| HuggingFaceError::Valtron(e.to_string()))?;

    match collect_one(stream) {
        Some(Ok(value)) => Ok(value),
        Some(Err(e)) => Err(e),
        None => Ok(false),
    }
}

/// Check if revision exists.
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

    let http_client = repo.inner.client.simple_http();

    let builder = http_client
        .get(&url)
        .map_err(|e| HuggingFaceError::Backend(e.to_string()))?
        .header(SimpleHeader::USER_AGENT, constants::HF_USER_AGENT);

    let builder = {
        if let Some(ref token) = repo.inner.client.token() {
            if !HFClient::is_implicit_token_disabled() {
                builder.header(SimpleHeader::AUTHORIZATION, format!("Bearer {}", token))
            } else {
                builder
            }
        } else {
            builder
        }
    };

    let task = builder
        .build_send_request()
        .map_err(|e| HuggingFaceError::Backend(e.to_string()))?
        .map_ready(move |intro| match intro {
            RequestIntro::Success { intro, .. } => {
                let status = &intro.0;
                Ok(is_success_status(status))
            }
            RequestIntro::Failed(_) => Ok(false),
        })
        .map_pending(|_| ());

    let stream = execute(task, None)
        .map_err(|e| HuggingFaceError::Valtron(e.to_string()))?;

    match collect_one(stream) {
        Some(Ok(value)) => Ok(value),
        Some(Err(e)) => Err(e),
        None => Ok(false),
    }
}

/// List tree entries.
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

    let http_client = repo.inner.client.simple_http();

    let builder = http_client
        .get(&url)
        .map_err(|e| HuggingFaceError::Backend(e.to_string()))?
        .header(SimpleHeader::USER_AGENT, constants::HF_USER_AGENT);

    let builder = {
        if let Some(ref token) = repo.inner.client.token() {
            if !HFClient::is_implicit_token_disabled() {
                builder.header(SimpleHeader::AUTHORIZATION, format!("Bearer {}", token))
            } else {
                builder
            }
        } else {
            builder
        }
    };

    let task = builder
        .build_send_request()
        .map_err(|e| HuggingFaceError::Backend(e.to_string()))?
        .map_ready(move |intro| match intro {
            RequestIntro::Success { stream, intro, .. } => {
                let status = &intro.0;
                if !is_success_status(status) {
                    return Err(HuggingFaceError::Http {
                        status: status_code(status),
                        url: url.clone(),
                        body: format!("HTTP {}", status_code(status)),
                    });
                }
                let body = body_reader::collect_string(stream);
                let entries: Vec<RepoTreeEntry> = serde_json::from_str(&body)
                    .map_err(HuggingFaceError::Json)?;
                Ok(entries)
            }
            RequestIntro::Failed(e) => Err(HuggingFaceError::Backend(format!(
                "Request failed: {}",
                e
            ))),
        })
        .map_pending(|_| ());

    let stream = execute(task, None)
        .map_err(|e| HuggingFaceError::Valtron(e.to_string()))?;

    Ok(stream.flat_map_next(|result| match result {
        Ok(entries) => {
            let v: Vec<_> = Iterator::collect(entries.into_iter().map(Ok));
            v.into_iter()
        }
        Err(e) => {
            let v: Vec<_> = Iterator::collect(vec![Err(e)].into_iter());
            v.into_iter()
        }
    }))
}

/// Download a file.
pub fn repo_download_file(repo: &HFRepository, params: &RepoDownloadFileParams) -> Result<PathBuf> {
    let revision = params.revision.as_deref().unwrap_or("main");
    let url = repo.inner.client.download_url(
        Some(repo.inner.repo_type),
        &repo.repo_path(),
        revision,
        &params.filename,
    );

    // Build destination path: directory/filename
    let destination = params.directory.join(&params.filename);

    let http_client = repo.inner.client.simple_http();

    let builder = http_client
        .get(&url)
        .map_err(|e| HuggingFaceError::Backend(e.to_string()))?
        .header(SimpleHeader::USER_AGENT, constants::HF_USER_AGENT);

    let builder = {
        if let Some(ref token) = repo.inner.client.token() {
            if !HFClient::is_implicit_token_disabled() {
                builder.header(SimpleHeader::AUTHORIZATION, format!("Bearer {}", token))
            } else {
                builder
            }
        } else {
            builder
        }
    };

    let task = builder
        .build_send_request()
        .map_err(|e| HuggingFaceError::Backend(e.to_string()))?
        .map_ready(move |intro| match intro {
            RequestIntro::Success { stream, intro, .. } => {
                let status = &intro.0;
                if !is_success_status(status) {
                    return Err(HuggingFaceError::Http {
                        status: status_code(status),
                        url: url.clone(),
                        body: format!("HTTP {}", status_code(status)),
                    });
                }
                let bytes = body_reader::collect_bytes(stream);
                std::fs::write(&destination, &bytes)
                    .map_err(HuggingFaceError::Io)?;
                Ok(destination.clone())
            }
            RequestIntro::Failed(e) => Err(HuggingFaceError::Backend(format!(
                "Request failed: {}",
                e
            ))),
        })
        .map_pending(|_| ());

    let stream = execute(task, None)
        .map_err(|e| HuggingFaceError::Valtron(e.to_string()))?;

    collect_one(stream)
        .ok_or_else(|| HuggingFaceError::Backend("No result from repo_download_file".into()))?
}

/// Upload a file.
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
pub fn repo_create_commit(repo: &HFRepository, params: &RepoCreateCommitParams) -> Result<CommitInfo> {
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

    // Build multipart body
    let boundary = "----RustBoundary".to_string();
    let mut body = String::new();

    body.push_str(&format!("--{}\r\n", boundary));
    body.push_str("Content-Disposition: form-data; name=\"summary\"\r\n\r\n");
    body.push_str(&serde_json::to_string(&serde_json::json!({
        "type": "commit",
        "summary": params.commit_message,
        "description": params.commit_description,
        "hub": "huggingface.co",
    }))?);
    body.push_str("\r\n");

    for (_i, op) in params.operations.iter().enumerate() {
        match op {
            CommitOperation::Add { path_in_repo, source } => {
                body.push_str(&format!("--{}\r\n", boundary));
                body.push_str(&format!(
                    "Content-Disposition: form-data; name=\"files\"; filename=\"{}\"\r\n",
                    path_in_repo
                ));
                body.push_str("Content-Type: application/octet-stream\r\n\r\n");
                match source {
                    AddSource::Bytes(data) => {
                        body.push_str(&String::from_utf8_lossy(data));
                    }
                    AddSource::File(path) => {
                        let content = std::fs::read_to_string(path)
                            .map_err(HuggingFaceError::Io)?;
                        body.push_str(&content);
                    }
                }
                body.push_str("\r\n");
            }
            CommitOperation::Delete { path_in_repo } => {
                body.push_str(&format!("--{}\r\n", boundary));
                body.push_str("Content-Disposition: form-data; name=\"deletedFiles\"\r\n\r\n");
                body.push_str(path_in_repo);
                body.push_str("\r\n");
            }
        }
    }

    body.push_str(&format!("--{}--\r\n", boundary));

    let http_client = repo.inner.client.simple_http();

    let content_type = format!("multipart/form-data; boundary={}", boundary);
    let builder = http_client
        .post(&url)
        .map_err(|e| HuggingFaceError::Backend(e.to_string()))?
        .header(SimpleHeader::USER_AGENT, constants::HF_USER_AGENT);

    let builder = {
        if let Some(ref token) = repo.inner.client.token() {
            if !HFClient::is_implicit_token_disabled() {
                builder.header(SimpleHeader::AUTHORIZATION, format!("Bearer {}", token))
            } else {
                builder
            }
        } else {
            builder
        }
    };

    let builder = builder
        .header(SimpleHeader::CONTENT_TYPE, content_type)
        .body_text(body);

    let task = builder
        .build_send_request()
        .map_err(|e| HuggingFaceError::Backend(e.to_string()))?
        .map_ready(move |intro| match intro {
            RequestIntro::Success { stream, intro, .. } => {
                let status = &intro.0;
                if !is_success_status(status) {
                    return Err(HuggingFaceError::Http {
                        status: status_code(status),
                        url: url.clone(),
                        body: format!("HTTP {}", status_code(status)),
                    });
                }
                let body = body_reader::collect_string(stream);
                let commit_info: CommitInfo = serde_json::from_str(&body)
                    .map_err(HuggingFaceError::Json)?;
                Ok(commit_info)
            }
            RequestIntro::Failed(e) => Err(HuggingFaceError::Backend(format!(
                "Request failed: {}",
                e
            ))),
        })
        .map_pending(|_| ());

    let stream = execute(task, None)
        .map_err(|e| HuggingFaceError::Valtron(e.to_string()))?;

    collect_one(stream)
        .ok_or_else(|| HuggingFaceError::Backend("No result from repo_create_commit".into()))?
}
