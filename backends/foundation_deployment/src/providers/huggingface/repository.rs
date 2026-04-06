//! Repository handle for Hugging Face Hub operations.

use crate::providers::huggingface::client::HFClient;
use crate::providers::huggingface::error::{HuggingFaceError, Result};
use crate::providers::huggingface::types::{
    CommitInfo, CommitOperation, RepoCreateCommitParams, RepoDeleteFileParams, RepoDownloadFileParams,
    RepoInfo, RepoInfoParams, RepoListTreeParams, RepoTreeEntry, RepoType, RepoUploadFileParams,
};
use foundation_core::valtron::{execute, Stream, StreamIteratorExt, TaskIteratorExt};
use foundation_core::wire::simple_http::client::{body_reader, RequestIntro};
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
    default_revision: Option<String>,
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
                default_revision: None,
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

    /// Get repository info.
    pub fn info(&self, params: &RepoInfoParams) -> Result<RepoInfo> {
        let client = self.inner.client.clone();
        let url = match self.inner.repo_type {
            RepoType::Dataset => format!(
                "{}/api/datasets/{}",
                self.inner.client.inner.endpoint,
                self.repo_path()
            ),
            RepoType::Space => format!(
                "{}/api/spaces/{}",
                self.inner.client.inner.endpoint,
                self.repo_path()
            ),
            _ => format!(
                "{}/api/models/{}",
                self.inner.client.inner.endpoint,
                self.repo_path()
            ),
        };

        let url = if let Some(ref revision) = params.revision {
            format!("{}/revision/{}", url, revision)
        } else {
            url
        };

        let headers = self.inner.client.auth_headers();

        let builder = client
            .get(&url)
            .map_err(|e| HuggingFaceError::HttpParse(e.into()))?
            .headers(headers);

        let task = builder
            .build_send_request()
            .map_err(|e| HuggingFaceError::HttpParse(e.into()))?
            .map_ready(|intro| match intro {
                RequestIntro::Success { stream, status } => {
                    if !status.is_success() {
                        return Err(HuggingFaceError::Http {
                            status: status.as_u16(),
                            url: url.clone(),
                            body: format!("HTTP {}", status.as_u16()),
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

        stream
            .find_map(|s| match s {
                Stream::Next(result) => Some(result),
                _ => None,
            })
            .ok_or_else(|| HuggingFaceError::Backend("No result from repo info".into()))
    }

    /// Check if repository exists.
    pub fn exists(&self) -> Result<bool> {
        let client = self.inner.client.clone();
        let url = match self.inner.repo_type {
            RepoType::Dataset => format!(
                "{}/api/datasets/{}",
                self.inner.client.inner.endpoint,
                self.repo_path()
            ),
            RepoType::Space => format!(
                "{}/api/spaces/{}",
                self.inner.client.inner.endpoint,
                self.repo_path()
            ),
            _ => format!(
                "{}/api/models/{}",
                self.inner.client.inner.endpoint,
                self.repo_path()
            ),
        };

        let headers = self.inner.client.auth_headers();

        let builder = client
            .get(&url)
            .map_err(|e| HuggingFaceError::HttpParse(e.into()))?
            .headers(headers);

        let task = builder
            .build_send_request()
            .map_err(|e| HuggingFaceError::HttpParse(e.into()))?
            .map_ready(|intro| match intro {
                RequestIntro::Success { status, .. } => Ok(status.is_success()),
                RequestIntro::Failed(_) => Ok(false),
            })
            .map_pending(|_| ());

        let stream = execute(task, None)
            .map_err(|e| HuggingFaceError::Valtron(e.to_string()))?;

        Ok(stream
            .find_map(|s| match s {
                Stream::Next(result) => Some(result),
                _ => None,
            })
            .unwrap_or(false))
    }

    /// Check if revision exists.
    pub fn revision_exists(&self, revision: &str) -> Result<bool> {
        let client = self.inner.client.clone();
        let url = match self.inner.repo_type {
            RepoType::Dataset => format!(
                "{}/api/datasets/{}/revision/{}",
                self.inner.client.inner.endpoint,
                self.repo_path(),
                revision
            ),
            RepoType::Space => format!(
                "{}/api/spaces/{}/revision/{}",
                self.inner.client.inner.endpoint,
                self.repo_path(),
                revision
            ),
            _ => format!(
                "{}/api/models/{}/revision/{}",
                self.inner.client.inner.endpoint,
                self.repo_path(),
                revision
            ),
        };

        let headers = self.inner.client.auth_headers();

        let builder = client
            .get(&url)
            .map_err(|e| HuggingFaceError::HttpParse(e.into()))?
            .headers(headers);

        let task = builder
            .build_send_request()
            .map_err(|e| HuggingFaceError::HttpParse(e.into()))?
            .map_ready(|intro| match intro {
                RequestIntro::Success { status, .. } => Ok(status.is_success()),
                RequestIntro::Failed(_) => Ok(false),
            })
            .map_pending(|_| ());

        let stream = execute(task, None)
            .map_err(|e| HuggingFaceError::Valtron(e.to_string()))?;

        Ok(stream
            .find_map(|s| match s {
                Stream::Next(result) => Some(result),
                _ => None,
            })
            .unwrap_or(false))
    }

    /// List tree entries.
    pub fn list_tree(
        &self,
        params: &RepoListTreeParams,
    ) -> Result<impl Iterator<Item = Stream<Result<RepoTreeEntry>, ()>> + Send + 'static> {
        let client = self.inner.client.clone();
        let base_path = match self.inner.repo_type {
            RepoType::Dataset => "datasets",
            RepoType::Space => "spaces",
            _ => "models",
        };
        let revision = params.revision.as_deref().unwrap_or("main");
        let url = format!(
            "{}/api/{}/{}/{}/tree/{}",
            self.inner.client.inner.endpoint,
            base_path,
            self.repo_path(),
            revision
        );

        let headers = self.inner.client.auth_headers();

        let builder = client
            .get(&url)
            .map_err(|e| HuggingFaceError::HttpParse(e.into()))?
            .headers(headers);

        let task = builder
            .build_send_request()
            .map_err(|e| HuggingFaceError::HttpParse(e.into()))?
            .map_ready(|intro| match intro {
                RequestIntro::Success { stream, status } => {
                    if !status.is_success() {
                        return Err(HuggingFaceError::Http {
                            status: status.as_u16(),
                            url: url.clone(),
                            body: format!("HTTP {}", status.as_u16()),
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
            Ok(entries) => entries
                .into_iter()
                .map(|e| Stream::Next(Ok(e)))
                .collect::<Vec<_>>()
                .into_iter(),
            Err(e) => vec![Stream::Next(Err(e))].into_iter(),
        }))
    }

    /// Download a file.
    pub fn download_file(&self, params: &RepoDownloadFileParams) -> Result<PathBuf> {
        let client = self.inner.client.clone();
        let revision = params.revision.as_deref().unwrap_or("main");
        let url = self.inner.client.download_url(
            Some(self.inner.repo_type),
            &self.repo_path(),
            revision,
            &params.filename,
        );

        let headers = self.inner.client.auth_headers();

        let builder = client
            .get(&url)
            .map_err(|e| HuggingFaceError::HttpParse(e.into()))?
            .headers(headers);

        let task = builder
            .build_send_request()
            .map_err(|e| HuggingFaceError::HttpParse(e.into()))?
            .map_ready(|intro| match intro {
                RequestIntro::Success { stream, status } => {
                    if !status.is_success() {
                        return Err(HuggingFaceError::Http {
                            status: status.as_u16(),
                            url: url.clone(),
                            body: format!("HTTP {}", status.as_u16()),
                        });
                    }
                    let body = body_reader::collect_string(stream);
                    let destination = params.destination.clone().unwrap_or_else(|| {
                        PathBuf::from(&params.filename)
                    });
                    std::fs::write(&destination, body.as_bytes())
                        .map_err(HuggingFaceError::Io)?;
                    Ok(destination)
                }
                RequestIntro::Failed(e) => Err(HuggingFaceError::Backend(format!(
                    "Request failed: {}",
                    e
                ))),
            })
            .map_pending(|_| ());

        let stream = execute(task, None)
            .map_err(|e| HuggingFaceError::Valtron(e.to_string()))?;

        stream
            .find_map(|s| match s {
                Stream::Next(result) => Some(result),
                _ => None,
            })
            .ok_or_else(|| HuggingFaceError::Backend("No result from download_file".into()))
    }

    /// Upload a file.
    pub fn upload_file(&self, params: &RepoUploadFileParams) -> Result<CommitInfo> {
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
        self.create_commit(&commit_params)
    }

    /// Delete a file.
    pub fn delete_file(&self, params: &RepoDeleteFileParams) -> Result<CommitInfo> {
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
        self.create_commit(&commit_params)
    }

    /// Create a commit with multiple operations.
    pub fn create_commit(&self, params: &RepoCreateCommitParams) -> Result<CommitInfo> {
        let client = self.inner.client.clone();
        let base_path = match self.inner.repo_type {
            RepoType::Dataset => "datasets",
            RepoType::Space => "spaces",
            _ => "models",
        };
        let revision = params.revision.as_deref().unwrap_or("main");
        let url = format!(
            "{}/api/{}/{}/commit/{}",
            self.inner.client.inner.endpoint,
            base_path,
            self.repo_path(),
            revision
        );

        let headers = self.inner.client.auth_headers();

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

        for (i, op) in params.operations.iter().enumerate() {
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
                    body.push_str(&format!(
                        "Content-Disposition: form-data; name=\"deletedFiles\"\r\n\r\n"
                    ));
                    body.push_str(path_in_repo);
                    body.push_str("\r\n");
                }
            }
        }

        body.push_str(&format!("--{}--\r\n", boundary));

        let builder = client
            .post(&url)
            .map_err(|e| HuggingFaceError::HttpParse(e.into()))?
            .headers(headers)
            .header("Content-Type", format!("multipart/form-data; boundary={}", boundary));

        let task = builder
            .body(body)
            .build_send_request()
            .map_err(|e| HuggingFaceError::HttpParse(e.into()))?
            .map_ready(|intro| match intro {
                RequestIntro::Success { stream, status } => {
                    if !status.is_success() {
                        return Err(HuggingFaceError::Http {
                            status: status.as_u16(),
                            url: url.clone(),
                            body: format!("HTTP {}", status.as_u16()),
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

        stream
            .find_map(|s| match s {
                Stream::Next(result) => Some(result),
                _ => None,
            })
            .ok_or_else(|| HuggingFaceError::Backend("No result from create_commit".into()))
    }
}

use crate::providers::huggingface::types::AddSource;
