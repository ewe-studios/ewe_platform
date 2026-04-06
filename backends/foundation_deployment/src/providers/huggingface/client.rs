//! Hugging Face Hub API client.

use crate::providers::huggingface::constants::{
    HF_API_DATASETS, HF_API_MODELS, HF_API_REPOS_CREATE, HF_API_REPOS_DELETE, HF_API_REPOS_MOVE,
    HF_API_SPACES, HF_API_WHOAMI, HF_DEFAULT_ENDPOINT, HF_HUB_DISABLE_IMPLICIT_TOKEN_ENV,
    HF_HOME_ENV, HF_TOKEN_ENV, HF_TOKEN_FILENAME, HF_TOKEN_PATH_ENV, HF_USER_AGENT,
};
use crate::providers::huggingface::error::{HuggingFaceError, Result};
use crate::providers::huggingface::types::{
    CommitInfo, CreateRepoParams, DatasetInfo, DeleteRepoParams, ListDatasetsParams,
    ListModelsParams, ListSpacesParams, ModelInfo, MoveRepoParams, RepoInfo, RepoType, SpaceInfo,
    User,
};
use foundation_core::valtron::{execute, Stream, StreamIteratorExt, TaskIteratorExt};
use foundation_core::wire::simple_http::client::{
    body_reader, ClientRequestBuilder, RequestIntro, SimpleHttpClient, SystemDnsResolver,
};
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
    cache_dir: PathBuf,
    cache_enabled: bool,
}

/// Builder for HFClient.
pub struct HFClientBuilder {
    endpoint: Option<String>,
    token: Option<String>,
    cache_dir: Option<PathBuf>,
    cache_enabled: Option<bool>,
}

impl Default for HFClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl HFClientBuilder {
    /// Create a new builder with defaults from environment.
    pub fn new() -> Self {
        Self {
            endpoint: None,
            token: None,
            cache_dir: None,
            cache_enabled: None,
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

    /// Set the cache directory.
    pub fn cache_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.cache_dir = Some(path.into());
        self
    }

    /// Enable or disable caching.
    pub fn cache_enabled(mut self, enabled: bool) -> Self {
        self.cache_enabled = Some(enabled);
        self
    }

    /// Build the client.
    pub fn build(self) -> Result<HFClient> {
        let endpoint = self
            .endpoint
            .unwrap_or_else(|| std::env::var("HF_ENDPOINT").unwrap_or_else(|_| HF_DEFAULT_ENDPOINT.to_string()));

        let token = self.token.or_else(|| resolve_token());

        let cache_dir = self.cache_dir.unwrap_or_else(|| {
            let home = std::env::var(HF_HOME_ENV)
                .unwrap_or_else(|_| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).to_string_lossy().to_string());
            PathBuf::from(home).join(".cache/huggingface")
        });

        let cache_enabled = self.cache_enabled.unwrap_or(true);

        let client = SimpleHttpClient::from_system();

        Ok(HFClient {
            inner: Arc::new(HFClientInner {
                client,
                endpoint,
                token,
                cache_dir,
                cache_enabled,
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

impl HFClient {
    /// Create a new client with defaults from environment.
    pub fn new() -> Result<Self> {
        HFClientBuilder::new().build()
    }

    /// Get a builder for fine-grained configuration.
    pub fn builder() -> HFClientBuilder {
        HFClientBuilder::new()
    }

    /// Get authenticated user info.
    ///
    /// Blocks internally — acceptable for single-value ops where result is needed immediately.
    pub fn whoami(&self) -> Result<User> {
        let client = self.inner.client.clone();
        let url = format!("{}{}", self.inner.endpoint, HF_API_WHOAMI);
        let headers = self.auth_headers();

        let builder = client
            .get(&url)
            .map_err(|e| HuggingFaceError::HttpParse(e.into()))?
            .headers(headers);

        let task = builder
            .build_send_request()
            .map_err(|e| HuggingFaceError::HttpParse(e.into()))?
            .map_ready(|intro| match intro {
                RequestIntro::Success { stream, status } => {
                    let headers = status.headers().clone();
                    if !status.is_success() {
                        return Err(HuggingFaceError::Http {
                            status: status.as_u16(),
                            url: url.clone(),
                            body: format!("HTTP {}", status.as_u16()),
                        });
                    }
                    let body = body_reader::collect_string(stream);
                    let user: User = serde_json::from_str(&body)
                        .map_err(HuggingFaceError::Json)?;
                    Ok(user)
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
            .ok_or_else(|| HuggingFaceError::Backend("No result from whoami".into()))
    }

    /// Check if token is valid.
    pub fn auth_check(&self) -> Result<()> {
        self.whoami().map(|_| ())
    }

    /// List models with pagination.
    ///
    /// Returns stream — caller composes and collects at boundary.
    pub fn list_models(
        &self,
        params: &ListModelsParams,
    ) -> Result<impl Iterator<Item = Stream<Result<ModelInfo>, ()>> + Send + 'static> {
        let client = self.inner.client.clone();
        let url = build_list_url(&self.inner.endpoint, HF_API_MODELS, params);

        let builder = client
            .get(&url)
            .map_err(|e| HuggingFaceError::HttpParse(e.into()))?;

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
                    let models: Vec<ModelInfo> = serde_json::from_str(&body)
                        .map_err(HuggingFaceError::Json)?;
                    Ok(models)
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
            Ok(models) => models
                .into_iter()
                .map(|m| Stream::Next(Ok(m)))
                .collect::<Vec<_>>()
                .into_iter(),
            Err(e) => vec![Stream::Next(Err(e))].into_iter(),
        }))
    }

    /// List datasets with pagination.
    pub fn list_datasets(
        &self,
        params: &ListDatasetsParams,
    ) -> Result<impl Iterator<Item = Stream<Result<DatasetInfo>, ()>> + Send + 'static> {
        let client = self.inner.client.clone();
        let url = build_list_url(&self.inner.endpoint, HF_API_DATASETS, params);

        let builder = client
            .get(&url)
            .map_err(|e| HuggingFaceError::HttpParse(e.into()))?;

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
                    let datasets: Vec<DatasetInfo> = serde_json::from_str(&body)
                        .map_err(HuggingFaceError::Json)?;
                    Ok(datasets)
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
            Ok(datasets) => datasets
                .into_iter()
                .map(|d| Stream::Next(Ok(d)))
                .collect::<Vec<_>>()
                .into_iter(),
            Err(e) => vec![Stream::Next(Err(e))].into_iter(),
        }))
    }

    /// List spaces with pagination.
    pub fn list_spaces(
        &self,
        params: &ListSpacesParams,
    ) -> Result<impl Iterator<Item = Stream<Result<SpaceInfo>, ()>> + Send + 'static> {
        let client = self.inner.client.clone();
        let url = build_list_url(&self.inner.endpoint, HF_API_SPACES, params);

        let builder = client
            .get(&url)
            .map_err(|e| HuggingFaceError::HttpParse(e.into()))?;

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
                    let spaces: Vec<SpaceInfo> = serde_json::from_str(&body)
                        .map_err(HuggingFaceError::Json)?;
                    Ok(spaces)
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
            Ok(spaces) => spaces
                .into_iter()
                .map(|s| Stream::Next(Ok(s)))
                .collect::<Vec<_>>()
                .into_iter(),
            Err(e) => vec![Stream::Next(Err(e))].into_iter(),
        }))
    }

    /// Create a repository.
    pub fn create_repo(&self, params: &CreateRepoParams) -> Result<RepoUrl> {
        let client = self.inner.client.clone();
        let url = format!("{}{}", self.inner.endpoint, HF_API_REPOS_CREATE);
        let headers = self.auth_headers();

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

        let builder = client
            .post(&url)
            .map_err(|e| HuggingFaceError::HttpParse(e.into()))?
            .headers(headers)
            .header("Content-Type", "application/json");

        let task = builder
            .body(serde_json::to_string(&body_json).map_err(HuggingFaceError::Json)?)
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
                    let repo_url: RepoUrl = serde_json::from_str(&body)
                        .map_err(HuggingFaceError::Json)?;
                    Ok(repo_url)
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
            .ok_or_else(|| HuggingFaceError::Backend("No result from create_repo".into()))
    }

    /// Delete a repository.
    pub fn delete_repo(&self, params: &DeleteRepoParams) -> Result<()> {
        let client = self.inner.client.clone();
        let url = format!("{}{}", self.inner.endpoint, HF_API_REPOS_DELETE);
        let headers = self.auth_headers();

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

        let builder = client
            .delete(&url)
            .map_err(|e| HuggingFaceError::HttpParse(e.into()))?
            .headers(headers)
            .header("Content-Type", "application/json");

        let task = builder
            .body(serde_json::to_string(&body_json).map_err(HuggingFaceError::Json)?)
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
                    let _ = body_reader::collect_string(stream);
                    Ok(())
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
            .ok_or_else(|| HuggingFaceError::Backend("No result from delete_repo".into()))
    }

    /// Move/rename a repository.
    pub fn move_repo(&self, params: &MoveRepoParams) -> Result<RepoUrl> {
        let client = self.inner.client.clone();
        let url = format!("{}{}", self.inner.endpoint, HF_API_REPOS_MOVE);
        let headers = self.auth_headers();

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

        let builder = client
            .post(&url)
            .map_err(|e| HuggingFaceError::HttpParse(e.into()))?
            .headers(headers)
            .header("Content-Type", "application/json");

        let task = builder
            .body(serde_json::to_string(&body_json).map_err(HuggingFaceError::Json)?)
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
                    let repo_url: RepoUrl = serde_json::from_str(&body)
                        .map_err(HuggingFaceError::Json)?;
                    Ok(repo_url)
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
            .ok_or_else(|| HuggingFaceError::Backend("No result from move_repo".into()))
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

    /// Build authentication headers.
    pub fn auth_headers(&self) -> http::HeaderMap {
        let mut headers = http::HeaderMap::new();
        headers.insert(
            "User-Agent",
            http::HeaderValue::from_static(HF_USER_AGENT),
        );

        if let Some(ref token) = self.inner.token {
            // Check if implicit token is disabled
            let implicit_disabled = std::env::var(HF_HUB_DISABLE_IMPLICIT_TOKEN_ENV)
                .map(|v| v == "1" || v == "true")
                .unwrap_or(false);

            if !implicit_disabled {
                headers.insert(
                    "Authorization",
                    http::HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
                );
            }
        }

        headers
    }

    /// Build API URL for a repository.
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

/// Build URL for listing operations.
fn build_list_url<P>(endpoint: &str, base_path: &str, params: &P) -> String
where
    P: ListParams,
{
    let mut url = format!("{}{}", endpoint, base_path);
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
        query_parts.push(format!("full={}", full));
    }
    if let Some(card_data) = params.card_data() {
        query_parts.push(format!("cardData={}", card_data));
    }
    if let Some(fetch_config) = params.fetch_config() {
        query_parts.push(format!("config={}", fetch_config));
    }
    if let Some(limit) = params.limit() {
        query_parts.push(format!("limit={}", limit));
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

use crate::providers::huggingface::repository::HFRepository;
