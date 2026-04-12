//! Hugging Face Hub API client and core operations.
//!
//! This module provides the [`HFClient`] struct and core API operations.
//! Functions are organized to make it clear which operations work with a client.
//!
//! # Client Operations
//!
//! - [`whoami`] - Get authenticated user info
//! - [`auth_check`] - Verify token is valid
//! - [`list_models`] - List models with pagination
//! - [`list_datasets`] - List datasets with pagination
//! - [`list_spaces`] - List spaces with pagination
//! - [`create_repo`] - Create a new repository
//! - [`delete_repo`] - Delete a repository
//! - [`move_repo`] - Move/rename a repository
//!
//! # Repository Operations
//!
//! For repository-specific operations, see the [`repository`](super::repository) module.

use crate::providers::huggingface::constants::{
    HF_API_DATASETS, HF_API_MODELS, HF_API_REPOS_CREATE, HF_API_REPOS_DELETE, HF_API_REPOS_MOVE,
    HF_API_SPACES, HF_API_WHOAMI, HF_DEFAULT_ENDPOINT, HF_HUB_DISABLE_IMPLICIT_TOKEN_ENV,
    HF_HOME_ENV, HF_TOKEN_ENV, HF_TOKEN_FILENAME, HF_TOKEN_PATH_ENV, HF_USER_AGENT,
};
use crate::providers::huggingface::error::{HuggingFaceError, Result};
use crate::providers::huggingface::repository::HFRepository;
use crate::providers::huggingface::types::{
    CreateRepoParams, DatasetInfo, DeleteRepoParams, ListDatasetsParams, ListModelsParams,
    ListSpacesParams, ModelInfo, MoveRepoParams, RepoType, SpaceInfo, User, RepoUrl,
};
use foundation_core::valtron::{collect_one, execute, Stream, StreamIteratorExt, TaskIteratorExt};
use foundation_core::wire::simple_http::client::{
    body_reader, ClientRequestBuilder, DnsResolver, RequestIntro, SimpleHttpClient,
};
use foundation_core::wire::simple_http::{SimpleHeader, SimpleHeaders, Status};
use std::path::PathBuf;
use std::sync::Arc;

/// Hugging Face Hub API client.
///
/// Cheap to clone — uses Arc internally.
#[derive(Clone)]
pub struct HFClient {
    inner: Arc<HFClientInner>,
}

struct HFClientInner {
    client: SimpleHttpClient,
    endpoint: String,
    token: Option<String>,
}

/// Builder for `HFClient`.
pub struct HFClientBuilder {
    endpoint: Option<String>,
    token: Option<String>,
}

impl Default for HFClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl HFClientBuilder {
    /// Create a new builder with defaults from environment.
    #[must_use] 
    pub fn new() -> Self {
        Self {
            endpoint: None,
            token: None,
        }
    }

    /// Set the API endpoint.
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Set the authentication token.
    pub fn token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Build the client.
    pub fn build(self) -> Result<HFClient> {
        let endpoint = self
            .endpoint
            .unwrap_or_else(|| std::env::var("HF_ENDPOINT").unwrap_or_else(|_| HF_DEFAULT_ENDPOINT.to_string()));

        let token = self.token.or_else(resolve_token);

        // Configure client to preserve auth headers on redirects
        // HuggingFace redirects to CDN (cas-bridge.xethub.hf.co) for file downloads
        let client = SimpleHttpClient::from_system()
            .preserve_auth_on_redirect(true);

        Ok(HFClient {
            inner: Arc::new(HFClientInner {
                client,
                endpoint,
                token,
            }),
        })
    }
}

/// Resolve token from environment, file, or cache.
fn resolve_token() -> Option<String> {
    // Try HF_TOKEN environment variable first
    if let Ok(token) = std::env::var(HF_TOKEN_ENV) {
        return Some(token);
    }

    // Try HF_TOKEN_PATH file
    if let Ok(token_path) = std::env::var(HF_TOKEN_PATH_ENV) {
        if let Ok(token) = std::fs::read_to_string(&token_path) {
            return Some(token.trim().to_string());
        }
    }

    // Try default token location: $HF_HOME/token
    if let Ok(hf_home) = std::env::var(HF_HOME_ENV) {
        let token_path = PathBuf::from(hf_home).join(HF_TOKEN_FILENAME);
        if let Ok(token) = std::fs::read_to_string(&token_path) {
            return Some(token.trim().to_string());
        }
    }

    // Try ~/.cache/huggingface/token
    if let Some(home) = dirs::home_dir() {
        let token_path = home.join(".cache/huggingface").join(HF_TOKEN_FILENAME);
        if let Ok(token) = std::fs::read_to_string(&token_path) {
            return Some(token.trim().to_string());
        }
    }

    None
}

/// Check if status code indicates success (2xx).
#[inline]
pub(crate) fn is_success_status(status: &Status) -> bool {
    let code: usize = status.clone().into();
    (200..300).contains(&code)
}

/// Get status code as u16.
#[inline]
pub(crate) fn status_code(status: &Status) -> u16 {
    let code: usize = status.clone().into();
    code as u16
}

impl HFClient {
    /// Create a new client with defaults from environment.
    pub fn new() -> Result<Self> {
        HFClientBuilder::new().build()
    }

    /// Get a builder for fine-grained configuration.
    #[must_use] 
    pub fn builder() -> HFClientBuilder {
        HFClientBuilder::new()
    }

    /// Get repository handle for model operations.
    pub fn model(&self, owner: impl Into<String>, name: impl Into<String>) -> HFRepository {
        HFRepository::new(self.clone(), owner.into(), name.into(), RepoType::Model)
    }

    /// Get repository handle for dataset operations.
    pub fn dataset(&self, owner: impl Into<String>, name: impl Into<String>) -> HFRepository {
        HFRepository::new(self.clone(), owner.into(), name.into(), RepoType::Dataset)
    }

    /// Get repository handle for space operations.
    pub fn space(&self, owner: impl Into<String>, name: impl Into<String>) -> HFRepository {
        HFRepository::new(self.clone(), owner.into(), name.into(), RepoType::Space)
    }

    /// Get the endpoint URL.
    pub(crate) fn endpoint(&self) -> &str {
        &self.inner.endpoint
    }

    /// Get the token, if set.
    pub(crate) fn token(&self) -> Option<&str> {
        self.inner.token.as_deref()
    }

    /// Check if implicit token is disabled.
    pub(crate) fn is_implicit_token_disabled() -> bool {
        std::env::var(HF_HUB_DISABLE_IMPLICIT_TOKEN_ENV)
            .map(|v| v == "1" || v == "true")
            .unwrap_or(false)
    }

    /// Get a clone of the underlying `SimpleHttpClient`.
    pub(crate) fn simple_http(&self) -> SimpleHttpClient {
        self.inner.client.clone()
    }

    /// Build authentication headers.
    ///
    /// Note: This returns only auth-related headers. For use with a request builder,
    /// use `apply_auth_headers()` to add them without replacing the Host header.
    #[must_use] 
    pub fn auth_headers(&self) -> SimpleHeaders {
        let mut headers = SimpleHeaders::new();
        headers.insert(SimpleHeader::USER_AGENT, vec![HF_USER_AGENT.to_string()]);

        if let Some(ref token) = self.inner.token {
            // Check if implicit token is disabled
            let implicit_disabled = std::env::var(HF_HUB_DISABLE_IMPLICIT_TOKEN_ENV)
                .map(|v| v == "1" || v == "true")
                .unwrap_or(false);

            if !implicit_disabled {
                headers.insert(
                    SimpleHeader::AUTHORIZATION,
                    vec![format!("Bearer {}", token)],
                );
            }
        }

        headers
    }

    /// Apply authentication headers to a request builder.
    ///
    /// This adds headers individually to avoid replacing the Host header.
    #[must_use] 
    pub fn apply_auth_headers<T: DnsResolver + 'static>(
        &self,
        builder: ClientRequestBuilder<T>,
    ) -> ClientRequestBuilder<T> {
        let builder = builder.header(SimpleHeader::USER_AGENT, HF_USER_AGENT);

        if let Some(ref token) = self.inner.token {
            if Self::is_implicit_token_disabled() {
                builder
            } else {
                builder.header(SimpleHeader::AUTHORIZATION, format!("Bearer {token}"))
            }
        } else {
            builder
        }
    }

    /// Build API URL for a repository.
    #[must_use] 
    pub fn api_url(&self, repo_type: Option<RepoType>, repo_id: &str) -> String {
        let prefix = match repo_type {
            Some(RepoType::Dataset) => "datasets/",
            Some(RepoType::Space) => "spaces/",
            Some(RepoType::Kernel) => "kernels/",
            _ => "",
        };
        format!("{}/{}/{}", self.inner.endpoint, prefix, repo_id)
    }

    /// Build download URL for a file.
    #[must_use] 
    pub fn download_url(
        &self,
        repo_type: Option<RepoType>,
        repo_id: &str,
        revision: &str,
        filename: &str,
    ) -> String {
        let prefix = match repo_type {
            Some(RepoType::Dataset) => "datasets/",
            Some(RepoType::Space) => "spaces/",
            Some(RepoType::Kernel) => "kernels/",
            _ => "",
        };
        format!(
            "{}/{}{}/resolve/{}/{}",
            self.inner.endpoint.replace("/api", ""),
            prefix,
            repo_id,
            revision,
            filename
        )
    }
}

// ============================================================================
// Client API Functions
// ============================================================================
// Core API operations that work with an HFClient instance.

/// Get authenticated user info.
///
/// Blocks internally — acceptable for single-value ops where result is needed immediately.
pub fn whoami(client: &HFClient) -> Result<User> {
    let http_client = client.inner.client.clone();
    let url = format!("{}{}", client.inner.endpoint, HF_API_WHOAMI);

    let builder = http_client
        .get(&url)
        .map_err(|e| HuggingFaceError::Backend(e.to_string()))?
        .header(SimpleHeader::USER_AGENT, HF_USER_AGENT);

    let builder = if let Some(ref token) = client.inner.token {
        let implicit_disabled = std::env::var(HF_HUB_DISABLE_IMPLICIT_TOKEN_ENV)
            .map(|v| v == "1" || v == "true")
            .unwrap_or(false);
        if implicit_disabled {
            builder
        } else {
            builder.header(SimpleHeader::AUTHORIZATION, format!("Bearer {token}"))
        }
    } else {
        builder
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
                let user: User = serde_json::from_str(&body).map_err(HuggingFaceError::Json)?;
                Ok(user)
            }
            RequestIntro::Failed(e) => Err(HuggingFaceError::Backend(format!(
                "Request failed: {e}"
            ))),
        })
        .map_pending(|_| ());

    let stream = execute(task, None)
        .map_err(|e| HuggingFaceError::Valtron(e.to_string()))?;

    collect_one(stream)
        .ok_or_else(|| HuggingFaceError::Backend("No result from whoami".into()))?
}

/// Check if token is valid.
pub fn auth_check(client: &HFClient) -> Result<()> {
    whoami(client).map(|_| ())
}

/// List models with pagination.
///
/// Returns stream — caller composes and collects at boundary.
pub fn list_models(
    client: &HFClient,
    params: &ListModelsParams,
) -> Result<impl Iterator<Item = Stream<Result<ModelInfo>, ()>> + Send + 'static> {
    let http_client = client.inner.client.clone();
    let url = build_list_url(&client.inner.endpoint, HF_API_MODELS, params);

    let builder = http_client
        .get(&url)
        .map_err(|e| HuggingFaceError::Backend(e.to_string()))?;

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
                let models: Vec<ModelInfo> =
                    serde_json::from_str(&body).map_err(HuggingFaceError::Json)?;
                Ok(models)
            }
            RequestIntro::Failed(e) => Err(HuggingFaceError::Backend(format!(
                "Request failed: {e}"
            ))),
        })
        .map_pending(|_| ());

    let stream = execute(task, None)
        .map_err(|e| HuggingFaceError::Valtron(e.to_string()))?;

    Ok(stream.flat_map_next(|result| match result {
        Ok(models) => {
            let v: Vec<_> = Iterator::collect(models.into_iter().map(Ok));
            v.into_iter()
        }
        Err(e) => {
            let v: Vec<_> = Iterator::collect(vec![Err(e)].into_iter());
            v.into_iter()
        }
    }))
}

/// List datasets with pagination.
pub fn list_datasets(
    client: &HFClient,
    params: &ListDatasetsParams,
) -> Result<impl Iterator<Item = Stream<Result<DatasetInfo>, ()>> + Send + 'static> {
    let http_client = client.inner.client.clone();
    let url = build_list_url(&client.inner.endpoint, HF_API_DATASETS, params);

    let builder = http_client
        .get(&url)
        .map_err(|e| HuggingFaceError::Backend(e.to_string()))?;

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
                let datasets: Vec<DatasetInfo> =
                    serde_json::from_str(&body).map_err(HuggingFaceError::Json)?;
                Ok(datasets)
            }
            RequestIntro::Failed(e) => Err(HuggingFaceError::Backend(format!(
                "Request failed: {e}"
            ))),
        })
        .map_pending(|_| ());

    let stream = execute(task, None)
        .map_err(|e| HuggingFaceError::Valtron(e.to_string()))?;

    Ok(stream.flat_map_next(|result| match result {
        Ok(datasets) => {
            let v: Vec<_> = Iterator::collect(datasets.into_iter().map(Ok));
            v.into_iter()
        }
        Err(e) => {
            let v: Vec<_> = Iterator::collect(vec![Err(e)].into_iter());
            v.into_iter()
        }
    }))
}

/// List spaces with pagination.
pub fn list_spaces(
    client: &HFClient,
    params: &ListSpacesParams,
) -> Result<impl Iterator<Item = Stream<Result<SpaceInfo>, ()>> + Send + 'static> {
    let http_client = client.inner.client.clone();
    let url = build_list_url(&client.inner.endpoint, HF_API_SPACES, params);

    let builder = http_client
        .get(&url)
        .map_err(|e| HuggingFaceError::Backend(e.to_string()))?;

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
                let spaces: Vec<SpaceInfo> =
                    serde_json::from_str(&body).map_err(HuggingFaceError::Json)?;
                Ok(spaces)
            }
            RequestIntro::Failed(e) => Err(HuggingFaceError::Backend(format!(
                "Request failed: {e}"
            ))),
        })
        .map_pending(|_| ());

    let stream = execute(task, None)
        .map_err(|e| HuggingFaceError::Valtron(e.to_string()))?;

    Ok(stream.flat_map_next(|result| match result {
        Ok(spaces) => {
            let v: Vec<_> = Iterator::collect(spaces.into_iter().map(Ok));
            v.into_iter()
        }
        Err(e) => {
            let v: Vec<_> = Iterator::collect(vec![Err(e)].into_iter());
            v.into_iter()
        }
    }))
}

/// Create a repository.
pub fn create_repo(client: &HFClient, params: &CreateRepoParams) -> Result<RepoUrl> {
    let http_client = client.inner.client.clone();
    let url = format!("{}{}", client.inner.endpoint, HF_API_REPOS_CREATE);

    let body_json = serde_json::json!({
        "name": params.repo_id,
        "type": params.repo_type.map(|t| match t {
            RepoType::Model => "model",
            RepoType::Dataset => "dataset",
            RepoType::Space => "space",
            RepoType::Kernel => "kernel",
        }),
        "private": params.private,
        "existOk": params.exist_ok,
        "spaceSdk": params.space_sdk,
    });

    let builder = http_client
        .post(&url)
        .map_err(|e: foundation_core::wire::simple_http::HttpClientError| {
            HuggingFaceError::Backend(e.to_string())
        })?;

    let builder = client.apply_auth_headers(builder);

    let builder = builder
        .body_json(&body_json)
        .map_err(|e: foundation_core::wire::simple_http::HttpClientError| {
            HuggingFaceError::Backend(e.to_string())
        })?;

    let task = builder
        .build_send_request()
        .map_err(|e: foundation_core::wire::simple_http::HttpClientError| {
            HuggingFaceError::Backend(e.to_string())
        })?
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
                let repo_url: RepoUrl =
                    serde_json::from_str(&body).map_err(HuggingFaceError::Json)?;
                Ok(repo_url)
            }
            RequestIntro::Failed(e) => Err(HuggingFaceError::Backend(format!(
                "Request failed: {e}"
            ))),
        })
        .map_pending(|_| ());

    let stream = execute(task, None)
        .map_err(|e| HuggingFaceError::Valtron(e.to_string()))?;

    collect_one(stream)
        .ok_or_else(|| HuggingFaceError::Backend("No result from create_repo".into()))?
}

/// Delete a repository.
pub fn delete_repo(client: &HFClient, params: &DeleteRepoParams) -> Result<()> {
    let http_client = client.inner.client.clone();
    let url = format!("{}{}", client.inner.endpoint, HF_API_REPOS_DELETE);

    let body_json = serde_json::json!({
        "name": params.repo_id,
        "type": params.repo_type.map(|t| match t {
            RepoType::Model => "model",
            RepoType::Dataset => "dataset",
            RepoType::Space => "space",
            RepoType::Kernel => "kernel",
        }),
        "missingOk": params.missing_ok,
    });

    let builder = http_client
        .delete(&url)
        .map_err(|e: foundation_core::wire::simple_http::HttpClientError| {
            HuggingFaceError::Backend(e.to_string())
        })?;

    let builder = client.apply_auth_headers(builder);

    let builder = builder
        .body_json(&body_json)
        .map_err(|e: foundation_core::wire::simple_http::HttpClientError| {
            HuggingFaceError::Backend(e.to_string())
        })?;

    let task = builder
        .build_send_request()
        .map_err(|e: foundation_core::wire::simple_http::HttpClientError| {
            HuggingFaceError::Backend(e.to_string())
        })?
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
                let _ = body_reader::collect_string(stream);
                Ok(())
            }
            RequestIntro::Failed(e) => Err(HuggingFaceError::Backend(format!(
                "Request failed: {e}"
            ))),
        })
        .map_pending(|_| ());

    let stream = execute(task, None)
        .map_err(|e| HuggingFaceError::Valtron(e.to_string()))?;

    collect_one(stream)
        .ok_or_else(|| HuggingFaceError::Backend("No result from delete_repo".into()))?
}

/// Move/rename a repository.
pub fn move_repo(client: &HFClient, params: &MoveRepoParams) -> Result<RepoUrl> {
    let http_client = client.inner.client.clone();
    let url = format!("{}{}", client.inner.endpoint, HF_API_REPOS_MOVE);

    let body_json = serde_json::json!({
        "from": params.from_id,
        "to": params.to_id,
        "type": params.repo_type.map(|t| match t {
            RepoType::Model => "model",
            RepoType::Dataset => "dataset",
            RepoType::Space => "space",
            RepoType::Kernel => "kernel",
        }),
    });

    let builder = http_client
        .post(&url)
        .map_err(|e: foundation_core::wire::simple_http::HttpClientError| {
            HuggingFaceError::Backend(e.to_string())
        })?;

    let builder = client.apply_auth_headers(builder);

    let builder = builder
        .body_json(&body_json)
        .map_err(|e: foundation_core::wire::simple_http::HttpClientError| {
            HuggingFaceError::Backend(e.to_string())
        })?;

    let task = builder
        .build_send_request()
        .map_err(|e: foundation_core::wire::simple_http::HttpClientError| {
            HuggingFaceError::Backend(e.to_string())
        })?
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
                let repo_url: RepoUrl =
                    serde_json::from_str(&body).map_err(HuggingFaceError::Json)?;
                Ok(repo_url)
            }
            RequestIntro::Failed(e) => Err(HuggingFaceError::Backend(format!(
                "Request failed: {e}"
            ))),
        })
        .map_pending(|_| ());

    let stream = execute(task, None)
        .map_err(|e| HuggingFaceError::Valtron(e.to_string()))?;

    collect_one(stream)
        .ok_or_else(|| HuggingFaceError::Backend("No result from move_repo".into()))?
}

/// Build URL for listing operations.
fn build_list_url<P>(endpoint: &str, base_path: &str, params: &P) -> String
where
    P: ListParams,
{
    let mut url = format!("{endpoint}{base_path}");
    let mut query_parts = Vec::new();

    if let Some(search) = params.search() {
        query_parts.push(format!("search={}", urlencoding::encode(search)));
    }
    if let Some(author) = params.author() {
        query_parts.push(format!("author={}", urlencoding::encode(author)));
    }
    if let Some(filter) = params.filter() {
        query_parts.push(format!("filter={}", urlencoding::encode(filter)));
    }
    if let Some(sort) = params.sort() {
        query_parts.push(format!("sort={}", urlencoding::encode(sort)));
    }
    if let Some(pipeline_tag) = params.pipeline_tag() {
        query_parts.push(format!(
            "pipeline_tag={}",
            urlencoding::encode(pipeline_tag)
        ));
    }
    if let Some(full) = params.full() {
        query_parts.push(format!("full={full}"));
    }
    if let Some(card_data) = params.card_data() {
        query_parts.push(format!("cardData={card_data}"));
    }
    if let Some(fetch_config) = params.fetch_config() {
        query_parts.push(format!("config={fetch_config}"));
    }
    if let Some(limit) = params.limit() {
        query_parts.push(format!("limit={limit}"));
    }

    if !query_parts.is_empty() {
        url.push('?');
        url.push_str(&query_parts.join("&"));
    }

    url
}

/// Trait for list parameters.
trait ListParams {
    fn search(&self) -> Option<&str>;
    fn author(&self) -> Option<&str>;
    fn filter(&self) -> Option<&str>;
    fn sort(&self) -> Option<&str>;
    fn pipeline_tag(&self) -> Option<&str>;
    fn full(&self) -> Option<bool>;
    fn card_data(&self) -> Option<bool>;
    fn fetch_config(&self) -> Option<bool>;
    fn limit(&self) -> Option<usize>;
}

impl ListParams for ListModelsParams {
    fn search(&self) -> Option<&str> {
        self.search.as_deref()
    }
    fn author(&self) -> Option<&str> {
        self.author.as_deref()
    }
    fn filter(&self) -> Option<&str> {
        self.filter.as_deref()
    }
    fn sort(&self) -> Option<&str> {
        self.sort.as_deref()
    }
    fn pipeline_tag(&self) -> Option<&str> {
        self.pipeline_tag.as_deref()
    }
    fn full(&self) -> Option<bool> {
        self.full
    }
    fn card_data(&self) -> Option<bool> {
        self.card_data
    }
    fn fetch_config(&self) -> Option<bool> {
        self.fetch_config
    }
    fn limit(&self) -> Option<usize> {
        self.limit
    }
}

impl ListParams for ListDatasetsParams {
    fn search(&self) -> Option<&str> {
        self.search.as_deref()
    }
    fn author(&self) -> Option<&str> {
        self.author.as_deref()
    }
    fn filter(&self) -> Option<&str> {
        self.filter.as_deref()
    }
    fn sort(&self) -> Option<&str> {
        self.sort.as_deref()
    }
    fn pipeline_tag(&self) -> Option<&str> {
        None
    }
    fn full(&self) -> Option<bool> {
        self.full
    }
    fn card_data(&self) -> Option<bool> {
        None
    }
    fn fetch_config(&self) -> Option<bool> {
        None
    }
    fn limit(&self) -> Option<usize> {
        self.limit
    }
}

impl ListParams for ListSpacesParams {
    fn search(&self) -> Option<&str> {
        self.search.as_deref()
    }
    fn author(&self) -> Option<&str> {
        self.author.as_deref()
    }
    fn filter(&self) -> Option<&str> {
        self.filter.as_deref()
    }
    fn sort(&self) -> Option<&str> {
        self.sort.as_deref()
    }
    fn pipeline_tag(&self) -> Option<&str> {
        None
    }
    fn full(&self) -> Option<bool> {
        self.full
    }
    fn card_data(&self) -> Option<bool> {
        None
    }
    fn fetch_config(&self) -> Option<bool> {
        None
    }
    fn limit(&self) -> Option<usize> {
        self.limit
    }
}
