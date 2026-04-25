//! Repository handle and operations for Hugging Face Hub.
//!
//! This module provides the [`HFRepository`] struct and repository-specific operations.
//! All functions take a `&HFRepository` as the first parameter.

#![allow(clippy::too_many_lines)]

use crate::providers::huggingface::client::{is_success_status, status_code, HFClient};
use crate::providers::huggingface::constants;
use crate::providers::huggingface::error::{HuggingFaceError, Result};
use crate::providers::huggingface::types::{
    AddSource, CommitInfo, CommitOperation, RepoCreateCommitParams, RepoDeleteFileParams,
    RepoDownloadFileParams, RepoInfo, RepoInfoParams, RepoListTreeParams, RepoTreeEntry, RepoType,
    RepoUploadFileParams,
};
use foundation_core::synca::RunOnDrop;
use foundation_core::valtron::{collect_one, execute, Stream, StreamIteratorExt, TaskIteratorExt};
use foundation_core::wire::simple_http::client::body_reader::{
    self, collect_bytes_into, collect_strings_from_send_safe,
};
use std::collections::BTreeMap;

use foundation_core::wire::simple_http::client::HttpClientConnection;
use foundation_core::wire::simple_http::client::{RequestIntro, ResponseIntro};
use foundation_core::wire::simple_http::SendSafeBody;
use foundation_core::wire::simple_http::{SimpleHeader, SimpleHeaders};
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
/// `RepoInfo`, or a transport/executor failure variant.
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

    let http_client = repo.inner.client.simple_http();

    let builder = http_client
        .get(&url)
        .map_err(|e| HuggingFaceError::Backend(e.to_string()))?
        .header(SimpleHeader::USER_AGENT, constants::HF_USER_AGENT);

    let builder = {
        if let Some(token) = repo.inner.client.token() {
            if HFClient::is_implicit_token_disabled() {
                builder
            } else {
                builder.header(SimpleHeader::AUTHORIZATION, format!("Bearer {}", &token))
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
                let info: RepoInfo = serde_json::from_str(&body).map_err(HuggingFaceError::Json)?;
                Ok(info)
            }
            RequestIntro::Failed(e) => {
                Err(HuggingFaceError::Backend(format!("Request failed: {e}")))
            }
        })
        .map_pending(|_| ());

    let stream = execute(task, None).map_err(|e| HuggingFaceError::Valtron(e.to_string()))?;

    collect_one(stream)
        .ok_or_else(|| HuggingFaceError::Backend("No result from repo_info".into()))?
}

/// Check if repository exists.
///
/// # Errors
///
/// Returns `HuggingFaceError::Backend` if the HTTP request cannot be built or
/// `HuggingFaceError::Valtron` if the executor cannot be scheduled. A 4xx
/// response from the Hub is mapped to `Ok(false)` rather than an error.
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
        if let Some(token) = repo.inner.client.token() {
            if HFClient::is_implicit_token_disabled() {
                builder
            } else {
                builder.header(SimpleHeader::AUTHORIZATION, format!("Bearer {}", &token))
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

    let stream = execute(task, None).map_err(|e| HuggingFaceError::Valtron(e.to_string()))?;

    match collect_one(stream) {
        Some(Ok(value)) => Ok(value),
        Some(Err(e)) => Err(e),
        None => Ok(false),
    }
}

/// Check if revision exists.
///
/// # Errors
///
/// Returns `HuggingFaceError::Backend` if the HTTP request cannot be built or
/// `HuggingFaceError::Valtron` if the executor cannot be scheduled. A 4xx
/// response from the Hub (missing revision) is mapped to `Ok(false)`.
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
        if let Some(token) = repo.inner.client.token() {
            if HFClient::is_implicit_token_disabled() {
                builder
            } else {
                builder.header(SimpleHeader::AUTHORIZATION, format!("Bearer {}", &token))
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

    let stream = execute(task, None).map_err(|e| HuggingFaceError::Valtron(e.to_string()))?;

    match collect_one(stream) {
        Some(Ok(value)) => Ok(value),
        Some(Err(e)) => Err(e),
        None => Ok(false),
    }
}

/// List tree entries.
///
/// # Errors
///
/// Returns `HuggingFaceError::Backend` if the HTTP request cannot be built or
/// `HuggingFaceError::Valtron` if the executor cannot be scheduled. Per-page
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

    let http_client = repo.inner.client.simple_http();

    let builder = http_client
        .get(&url)
        .map_err(|e| HuggingFaceError::Backend(e.to_string()))?
        .header(SimpleHeader::USER_AGENT, constants::HF_USER_AGENT);

    let builder = {
        if let Some(token) = repo.inner.client.token() {
            if HFClient::is_implicit_token_disabled() {
                builder
            } else {
                builder.header(SimpleHeader::AUTHORIZATION, format!("Bearer {}", &token))
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
                let entries: Vec<RepoTreeEntry> =
                    serde_json::from_str(&body).map_err(HuggingFaceError::Json)?;
                Ok(entries)
            }
            RequestIntro::Failed(e) => {
                Err(HuggingFaceError::Backend(format!("Request failed: {e}")))
            }
        })
        .map_pending(|_| ());

    let stream = execute(task, None).map_err(|e| HuggingFaceError::Valtron(e.to_string()))?;

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

/// Download a file, handling `HuggingFace`'s CDN redirect.
///
/// `HuggingFace` returns a 302 redirect to their CDN for file downloads.
/// This function handles that redirect manually.
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

    let http_client = repo.inner.client.simple_http();

    let builder = http_client
        .get(&url)
        .map_err(|e| HuggingFaceError::Backend(e.to_string()))?
        // .client_config_follow_other_redirects_response(false)
        .client_config_headers_to_add(BTreeMap::<SimpleHeader, Vec<String>>::from([(
            SimpleHeader::USER_AGENT,
            vec![constants::HF_USER_AGENT.to_string()],
        )]))
        .header(SimpleHeader::USER_AGENT, constants::HF_USER_AGENT);

    let builder = {
        if let Some(token) = repo.inner.client.token() {
            if HFClient::is_implicit_token_disabled() {
                println!("HF_TOKEN was disabled and will not be added!");
                tracing::debug!("HF_TOKEN was disabled and will not be added!");
                builder
            } else {
                println!("Adding HF_TOKEN with token having length={}", &token.len());
                tracing::debug!("Adding HF_TOKEN with token having length={}", &token.len());
                builder.header(SimpleHeader::AUTHORIZATION, format!("Bearer {}", &token))
            }
        } else {
            builder
        }
    };

    // First request - may return 302 redirect to CDN
    let mut request = builder
        .build_client()
        .map_err(|e| HuggingFaceError::Backend(e.to_string()))?;

    tracing::debug!("Call endpoint for huggingface={:?}", &params);

    let pool = request.get_pool();
    let (intro_stream, body_stream) = request
        .start()
        .map_err(|e| HuggingFaceError::Backend(e.to_string()))?;

    // Get intro (status + headers)
    let mut intro_data: Option<(ResponseIntro, SimpleHeaders)> = None;
    for intro_element in intro_stream {
        if let Stream::Next(value) = intro_element {
            intro_data = Some(value);
            break;
        }
    }

    let (intro, _headers) =
        intro_data.ok_or_else(|| HuggingFaceError::Backend("No response intro received".into()))?;

    let status = &intro.status;
    let status_num = status_code(status);

    tracing::debug!("Received initial response with status={}", &status);

    if status_num == 400 {
        // return Err(HuggingFaceError::Http {
        //     status: status_code(&response.get_status()),
        //     url: url.clone(),
        //     body: format!("HTTP {}", status_code(&response.get_status())),
        // });

        let mut response_body: Option<(HttpClientConnection, SendSafeBody)> = None;

        tracing::trace!("Read body stream for response body");
        for body_element in body_stream {
            if let Stream::Next(value) = body_element {
                let res = value.map_err(|e| HuggingFaceError::Backend(e.to_string()))?;
                response_body = Some(res);
                break;
            }
        }

        let Some((_, body)) = response_body else {
            return Err(HuggingFaceError::Http {
                status: status_num,
                url: url.clone(),
                body: format!("HTTP {} - Unable to get body", status_num),
            });
        };

        let response_body = collect_strings_from_send_safe(body)
            .map_err(|e| HuggingFaceError::Backend(e.to_string()))?;

        return Err(HuggingFaceError::Http {
            status: status_num,
            url: url.clone(),
            body: format!("HTTP {} - {}", status_num, response_body),
        });
    }

    let mut response_body: Option<(HttpClientConnection, SendSafeBody)> = None;
    for body_element in body_stream {
        if let Stream::Next(value) = body_element {
            response_body = Some(value.map_err(|e| HuggingFaceError::Backend(e.to_string()))?);
            break;
        }
    }

    let Some((conn, body)) = response_body else {
        return Err(HuggingFaceError::Backend(
            "Failed to get response body".into(),
        ));
    };

    let _guard = RunOnDrop::new(move || {
        pool.return_to_pool(conn);
    });

    // Stream body directly to file
    let mut file = std::fs::File::create(&destination).map_err(HuggingFaceError::Io)?;
    collect_bytes_into(body, &mut file).map_err(|e| HuggingFaceError::Backend(e.to_string()))?;

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
/// `HuggingFaceError::Backend`/`HuggingFaceError::Valtron` for transport and
/// executor failures.
pub fn repo_create_commit(
    repo: &HFRepository,
    params: &RepoCreateCommitParams,
) -> Result<CommitInfo> {
    use std::fmt::Write as _;

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

    let _ = write!(body, "--{boundary}\r\n");
    body.push_str("Content-Disposition: form-data; name=\"summary\"\r\n\r\n");
    body.push_str(&serde_json::to_string(&serde_json::json!({
        "type": "commit",
        "summary": params.commit_message,
        "description": params.commit_description,
        "hub": "huggingface.co",
    }))?);
    body.push_str("\r\n");

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
                body.push_str("Content-Type: application/octet-stream\r\n\r\n");
                match source {
                    AddSource::Bytes(data) => {
                        body.push_str(&String::from_utf8_lossy(data));
                    }
                    AddSource::File(path) => {
                        let content =
                            std::fs::read_to_string(path).map_err(HuggingFaceError::Io)?;
                        body.push_str(&content);
                    }
                }
                body.push_str("\r\n");
            }
            CommitOperation::Delete { path_in_repo } => {
                let _ = write!(body, "--{boundary}\r\n");
                body.push_str("Content-Disposition: form-data; name=\"deletedFiles\"\r\n\r\n");
                body.push_str(path_in_repo);
                body.push_str("\r\n");
            }
        }
    }

    let _ = write!(body, "--{boundary}--\r\n");

    let http_client = repo.inner.client.simple_http();

    let content_type = format!("multipart/form-data; boundary={boundary}");
    let builder = http_client
        .post(&url)
        .map_err(|e| HuggingFaceError::Backend(e.to_string()))?
        .header(SimpleHeader::USER_AGENT, constants::HF_USER_AGENT);

    let builder = {
        if let Some(token) = repo.inner.client.token() {
            if HFClient::is_implicit_token_disabled() {
                builder
            } else {
                builder.header(SimpleHeader::AUTHORIZATION, format!("Bearer {}", &token))
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
                let commit_info: CommitInfo =
                    serde_json::from_str(&body).map_err(HuggingFaceError::Json)?;
                Ok(commit_info)
            }
            RequestIntro::Failed(e) => {
                Err(HuggingFaceError::Backend(format!("Request failed: {e}")))
            }
        })
        .map_pending(|_| ());

    let stream = execute(task, None).map_err(|e| HuggingFaceError::Valtron(e.to_string()))?;

    collect_one(stream)
        .ok_or_else(|| HuggingFaceError::Backend("No result from repo_create_commit".into()))?
}
