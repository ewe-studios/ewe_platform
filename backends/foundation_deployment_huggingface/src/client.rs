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

use crate::constants::{
    HF_API_DATASETS, HF_API_MODELS, HF_API_REPOS_CREATE, HF_API_REPOS_DELETE, HF_API_REPOS_MOVE,
    HF_API_SPACES, HF_API_WHOAMI, HF_DEFAULT_ENDPOINT, HF_HOME_ENV,
    HF_HUB_DISABLE_IMPLICIT_TOKEN_ENV, HF_TOKEN_ENV, HF_TOKEN_FILENAME, HF_TOKEN_PATH_ENV,
    HF_USER_AGENT,
};
use crate::error::{HuggingFaceError, Result};
use crate::repository::HFRepository;
use crate::types::{
    CreateRepoParams, DatasetInfo, DeleteRepoParams, ListDatasetsParams, ListModelsParams,
    ListSpacesParams, ModelInfo, MoveRepoParams, RepoType, RepoUrl, SpaceInfo, User,
};
use foundation_core::valtron::Stream;
use foundation_netio::shared::client::body_reader::{collect_bytes_into, try_collect_bytes};
use foundation_netio::shared::client::request::PreparedRequest;
use foundation_netio::shared::http::{
    SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod, Status,
};
use foundation_netio::{DynNetClient, HttpClientBuilder, PreparedRequestBuilder};
use serde::de::DeserializeOwned;
use std::path::PathBuf;
use std::sync::Arc;

/// Default for [`HFClientBuilder::max_error_body`]: 64 KiB, the amount Go
/// `net/http` drains from a response body it discards.
pub const DEFAULT_MAX_ERROR_BODY: usize = 64 * 1024;

/// Hugging Face Hub API client.
///
/// Cheap to clone — uses Arc internally.
#[derive(Clone)]
pub struct HFClient {
    inner: Arc<HFClientInner>,
}

struct HFClientInner {
    client: DynNetClient,
    endpoint: String,
    token: Option<String>,
    max_error_body: usize,
}

/// Builder for `HFClient`.
pub struct HFClientBuilder {
    endpoint: Option<String>,
    token: Option<String>,
    client: Option<DynNetClient>,
    max_error_body: usize,
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
            client: None,
            max_error_body: DEFAULT_MAX_ERROR_BODY,
        }
    }

    /// Send the Hub requests through `client` (a shared client, or a test
    /// client) instead of the default one. The default keeps the
    /// `Authorization` header across the Hub's redirect to its CDN and sends
    /// the Hub user agent on every request, redirects included; a client
    /// given here should do the same.
    #[must_use]
    pub fn with_client(mut self, client: DynNetClient) -> Self {
        self.client = Some(client);
        self
    }

    /// Read at most `bytes` of an error answer (a non-2xx status) for the
    /// error message; the rest is never read. Default
    /// [`DEFAULT_MAX_ERROR_BODY`].
    #[must_use]
    pub fn max_error_body(mut self, bytes: usize) -> Self {
        self.max_error_body = bytes;
        self
    }

    /// Set the API endpoint.
    #[must_use]
    pub fn endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// Set the authentication token.
    #[must_use]
    pub fn token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Build the client.
    ///
    /// # Errors
    ///
    /// Currently infallible, but returns `Result` to allow future configuration
    /// validation (e.g., endpoint URL parsing) without a breaking change.
    pub fn build(self) -> Result<HFClient> {
        let endpoint = self.endpoint.unwrap_or_else(|| {
            std::env::var("HF_ENDPOINT").unwrap_or_else(|_| HF_DEFAULT_ENDPOINT.to_string())
        });

        let token = self.token.or_else(resolve_token);

        // Keep the auth header across redirects: the Hub redirects file
        // downloads to its CDN (cas-bridge.xethub.hf.co). The user agent is a
        // client default so redirected requests carry it too.
        let client = self.client.unwrap_or_else(|| {
            HttpClientBuilder::new()
                .preserve_auth_on_redirect(true)
                .default_header("User-Agent", HF_USER_AGENT)
                .build()
        });

        Ok(HFClient {
            inner: Arc::new(HFClientInner {
                client,
                endpoint,
                token,
                max_error_body: self.max_error_body,
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

/// Get status code as `u16`.
///
/// Saturates to `u16::MAX` if the underlying value exceeds the `u16` range
/// (which should never happen for HTTP status codes — they are always 100-599).
#[inline]
pub(crate) fn status_code(status: &Status) -> u16 {
    let code: usize = status.clone().into();
    u16::try_from(code).unwrap_or(u16::MAX)
}

impl HFClient {
    /// Create a new client with defaults from environment.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying builder fails to construct an
    /// `HFClient` (currently infallible, see `HFClientBuilder::build`).
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

    /// Check if implicit token is disabled.
    pub(crate) fn is_implicit_token_disabled() -> bool {
        std::env::var(HF_HUB_DISABLE_IMPLICIT_TOKEN_ENV).is_ok_and(|v| v == "1" || v == "true")
    }

    /// A request to `url` with the Hub user agent and, when `with_auth`, the
    /// bearer token (unless implicit tokens are disabled).
    pub(crate) fn request(
        &self,
        method: SimpleMethod,
        url: &str,
        with_auth: bool,
    ) -> Result<PreparedRequestBuilder> {
        let builder = PreparedRequestBuilder::new(method, url)
            .map_err(|e| HuggingFaceError::Backend(e.to_string()))?;
        Ok(if with_auth {
            self.apply_auth_headers(builder)
        } else {
            builder.header(SimpleHeader::USER_AGENT, HF_USER_AGENT)
        })
    }

    /// Send `request`. A transport failure is an error; the status is
    /// returned as is and the body is not read.
    pub(crate) fn send_raw(&self, request: PreparedRequest) -> Result<(Status, SendSafeBody)> {
        let response = self
            .inner
            .client
            .send(request)
            .map_err(|e| HuggingFaceError::Backend(format!("Request failed: {e}")))?;
        let (status, _headers, body) = response.into_parts();
        Ok((status, body))
    }

    /// Send `request` and return its body; an error unless the status is
    /// 2xx. The body of an error answer is read only up to `max_error_body`.
    pub(crate) fn send(&self, request: PreparedRequest, url: &str) -> Result<SendSafeBody> {
        let (status, body) = self.send_raw(request)?;
        if !is_success_status(&status) {
            return Err(self.http_error(&status, url, body));
        }
        Ok(body)
    }

    /// Send `request` and parse its (small) JSON answer.
    pub(crate) fn send_json<T: DeserializeOwned>(
        &self,
        request: PreparedRequest,
        url: &str,
    ) -> Result<T> {
        let body = self.send(request, url)?;
        let bytes = try_collect_bytes(body)
            .map_err(|e| HuggingFaceError::Backend(format!("read body from {url}: {e}")))?;
        serde_json::from_slice(&bytes).map_err(HuggingFaceError::Json)
    }

    /// The error for a non-2xx answer, with at most `max_error_body` bytes
    /// of its body.
    pub(crate) fn http_error(
        &self,
        status: &Status,
        url: &str,
        body: SendSafeBody,
    ) -> HuggingFaceError {
        let code = status_code(status);
        let mut limited = LimitedWriter::new(self.inner.max_error_body);
        // The writer refuses bytes past the limit, which ends the read early;
        // that error is expected and what was read is kept.
        let _ = collect_bytes_into(body, &mut limited);
        let message = String::from_utf8_lossy(&limited.buf);
        HuggingFaceError::Http {
            status: code,
            url: url.to_string(),
            body: if message.is_empty() {
                format!("HTTP {code}")
            } else {
                format!("HTTP {code} - {message}")
            },
        }
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
            if !Self::is_implicit_token_disabled() {
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
    pub fn apply_auth_headers(&self, builder: PreparedRequestBuilder) -> PreparedRequestBuilder {
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
/// Blocks until the answer arrives (native client, sync `send`).
///
/// # Errors
///
/// Returns `HuggingFaceError::Http` if the API responds with a non-2xx status,
/// `HuggingFaceError::Json` if the response body cannot be deserialized into
/// a `User`, or `HuggingFaceError::Backend` if the HTTP request fails.
pub fn whoami(client: &HFClient) -> Result<User> {
    let url = format!("{}{}", client.inner.endpoint, HF_API_WHOAMI);
    let request = client.request(SimpleMethod::GET, &url, true)?.build();
    client.send_json(request, &url)
}

/// Check if token is valid.
///
/// # Errors
///
/// Returns the same errors as `whoami` — a non-2xx HTTP response (typically
/// 401 if the token is invalid or missing), a transport failure, or a
/// deserialization failure on the user payload.
pub fn auth_check(client: &HFClient) -> Result<()> {
    whoami(client).map(|_| ())
}

/// List models with pagination.
///
/// The page is fetched before this returns; the iterator yields its entries.
///
/// # Errors
///
/// Returns `HuggingFaceError::Backend` if the HTTP request cannot be built
///. Per-page
/// errors (HTTP failures, JSON parse failures) surface as `Err` items inside
/// the returned stream.
pub fn list_models(
    client: &HFClient,
    params: &ListModelsParams,
) -> Result<impl Iterator<Item = Stream<Result<ModelInfo>, ()>> + Send + 'static> {
    let url = build_list_url(&client.inner.endpoint, HF_API_MODELS, params);
    let request = client.request(SimpleMethod::GET, &url, false)?.build();
    Ok(page_items(
        client.send_json::<Vec<ModelInfo>>(request, &url),
    ))
}

/// List datasets with pagination.
///
/// # Errors
///
/// Returns `HuggingFaceError::Backend` if the HTTP request cannot be built
///. Per-page
/// errors surface as `Err` items inside the returned stream.
pub fn list_datasets(
    client: &HFClient,
    params: &ListDatasetsParams,
) -> Result<impl Iterator<Item = Stream<Result<DatasetInfo>, ()>> + Send + 'static> {
    let url = build_list_url(&client.inner.endpoint, HF_API_DATASETS, params);
    let request = client.request(SimpleMethod::GET, &url, false)?.build();
    Ok(page_items(
        client.send_json::<Vec<DatasetInfo>>(request, &url),
    ))
}

/// List spaces with pagination.
///
/// # Errors
///
/// Returns `HuggingFaceError::Backend` if the HTTP request cannot be built
///. Per-page
/// errors surface as `Err` items inside the returned stream.
pub fn list_spaces(
    client: &HFClient,
    params: &ListSpacesParams,
) -> Result<impl Iterator<Item = Stream<Result<SpaceInfo>, ()>> + Send + 'static> {
    let url = build_list_url(&client.inner.endpoint, HF_API_SPACES, params);
    let request = client.request(SimpleMethod::GET, &url, false)?.build();
    Ok(page_items(
        client.send_json::<Vec<SpaceInfo>>(request, &url),
    ))
}

/// Create a repository.
///
/// # Errors
///
/// Returns `HuggingFaceError::Http` if the API responds with a non-2xx status
/// (e.g., 409 if the repo already exists and `exist_ok` is `false`),
/// `HuggingFaceError::Json` on malformed response bodies, or
/// `HuggingFaceError::Backend` for transport failures.
pub fn create_repo(client: &HFClient, params: &CreateRepoParams) -> Result<RepoUrl> {
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

    let request = client
        .request(SimpleMethod::POST, &url, true)?
        .body_json(&body_json)
        .map_err(|e| HuggingFaceError::Backend(e.to_string()))?
        .build();
    client.send_json(request, &url)
}

/// Delete a repository.
///
/// # Errors
///
/// Returns `HuggingFaceError::Http` if the API responds with a non-2xx status
/// (e.g., 404 when the repo does not exist and `missing_ok` is `false`), or
/// `HuggingFaceError::Backend` for transport failures.
pub fn delete_repo(client: &HFClient, params: &DeleteRepoParams) -> Result<()> {
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

    let request = client
        .request(SimpleMethod::DELETE, &url, true)?
        .body_json(&body_json)
        .map_err(|e| HuggingFaceError::Backend(e.to_string()))?
        .build();
    // The answer has no content we use; the body is dropped unread.
    client.send(request, &url).map(drop)
}

/// Move/rename a repository.
///
/// # Errors
///
/// Returns `HuggingFaceError::Http` if the API responds with a non-2xx status,
/// `HuggingFaceError::Json` on malformed response bodies, or
/// `HuggingFaceError::Backend` for transport failures.
pub fn move_repo(client: &HFClient, params: &MoveRepoParams) -> Result<RepoUrl> {
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

    let request = client
        .request(SimpleMethod::POST, &url, true)?
        .body_json(&body_json)
        .map_err(|e| HuggingFaceError::Backend(e.to_string()))?
        .build();
    client.send_json(request, &url)
}

/// One page of a listing as stream items: each entry, or the page's error.
fn page_items<T>(page: Result<Vec<T>>) -> std::vec::IntoIter<Stream<Result<T>, ()>> {
    let items: Vec<Stream<Result<T>, ()>> = match page {
        Ok(entries) => entries.into_iter().map(|e| Stream::Next(Ok(e))).collect(),
        Err(e) => vec![Stream::Next(Err(e))],
    };
    items.into_iter()
}

/// A writer that keeps at most `limit` bytes and refuses the rest.
struct LimitedWriter {
    buf: Vec<u8>,
    limit: usize,
}

impl LimitedWriter {
    fn new(limit: usize) -> Self {
        Self {
            buf: Vec::new(),
            limit,
        }
    }
}

impl std::io::Write for LimitedWriter {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        let room = self.limit - self.buf.len();
        if room == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "limit reached",
            ));
        }
        let n = data.len().min(room);
        self.buf.extend_from_slice(&data[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
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
