//! The registry client (feature 15, section 6.2).
//!
//! The client reads one manifest, then streams each blob to the blob cache
//! below `<cache_dir>/oci/blobs/sha256/`. Every blob is hashed while it
//! arrives, and the client renames it into place only when its digest and its
//! size match the manifest (invariant I1).
//!
//! The status is checked before the body on every call. A redirect is followed
//! by the HTTP client, up to `max_redirects`, and a redirect to another host is
//! sent with no `Authorization` header (invariant I4).

use std::fmt;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use foundation_compact::rng::Rng as _;
use foundation_compact::Instant;
use foundation_context::Ctx;
use foundation_core::valtron::{execute, StreamIteratorExt as _};
use foundation_netio::shared::client::request_task::HttpExchange;
use foundation_netio::shared::http::{SimpleHeader, SimpleHeaders};
use foundation_netio::{DynNetClient, HttpClientBuilder, PreparedRequestBuilder};
use futures::future::{select, Either};
use futures::{Stream, StreamExt as _};
use sha2::{Digest as _, Sha256};

use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::host::HostFs;
use crate::shared::spec::GuestArch;
use crate::shared::Secret;

use super::auth::{
    basic_header, check_realm, parse_www_authenticate, query_value, Challenge, TokenResponse,
};
use super::manifest::{
    check_image_arch, choose_platform, DocumentKind, OciDescriptor, OciImageConfigDocument,
    OciManifestDocument, MANIFEST_ACCEPT,
};
use super::reference::OciReference;

/// The directory below the cache directory that holds the verified blobs.
pub const BLOB_CACHE: &str = "oci";
/// The digest algorithm directory of the blob cache.
pub const BLOB_ALGORITHM: &str = "sha256";
/// The blob directory below [`BLOB_CACHE`].
pub const BLOB_DIR: &str = "blobs";

/// The result of one manifest resolution.
#[derive(Clone, Debug)]
pub struct ResolvedManifest {
    /// The digest of the manifest, `sha256:<64 lower-case hex>`.
    pub digest: String,
    /// The manifest document: its configuration and its layers.
    pub manifest: OciManifestDocument,
}

/// The credentials of one registry host.
#[derive(Clone)]
pub enum RegistryAuth {
    /// No credentials.
    Anonymous,
    /// A user name and a password.
    Basic {
        /// The user name.
        username: String,
        /// The password. Its `Debug` and `Display` forms are redacted.
        password: crate::shared::Secret,
    },
}

impl fmt::Debug for RegistryAuth {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Anonymous => formatter.write_str("Anonymous"),
            Self::Basic { .. } => formatter.write_str("Basic(<redacted>)"),
        }
    }
}

/// The options of one registry client.
#[derive(Clone, Debug, Default)]
pub struct RegistryOptions {
    /// The credentials of each registry host.
    pub auth: Vec<(String, RegistryAuth)>,
    /// The hosts that this client reaches with `http://` (invariant 4.5).
    pub allow_insecure: Vec<String>,
    /// The size and time limits.
    pub limits: RegistryLimits,
}

impl RegistryOptions {
    /// The credentials of `registry`, or `None` when the caller gave none.
    #[must_use]
    pub fn auth_for(&self, registry: &str) -> Option<&RegistryAuth> {
        self.auth
            .iter()
            .find(|(host, _)| host.eq_ignore_ascii_case(registry))
            .map(|(_, auth)| auth)
    }

    /// Whether this client reaches `registry` with `http://`.
    #[must_use]
    pub fn is_insecure(&self, registry: &str) -> bool {
        self.allow_insecure
            .iter()
            .any(|host| host.eq_ignore_ascii_case(registry))
    }
}

/// The size and time limits of one registry pull.
///
/// Each default names the source of the value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryLimits {
    /// The largest manifest body that the client reads.
    ///
    /// Default 4 MiB: the manifest limit of the Docker registry.
    pub max_manifest_bytes: usize,
    /// The most layers that one manifest may name.
    ///
    /// Default 127: the layer limit of the Docker overlay2 storage driver.
    pub max_layers: usize,
    /// The most redirects that one request follows.
    ///
    /// Default 5: the Go `net/http` default.
    pub max_redirects: u8,
    /// The largest blob that the client downloads.
    ///
    /// Default 4 GiB: a base image layer rarely passes this size, and a host
    /// has the free space for one such layer.
    pub max_blob_bytes: u64,
    /// The most bytes of a failed response body that an error carries.
    ///
    /// Default 64 KiB: the `BodyLimits::max_error_body` default of this
    /// repository, the same value as `SandboxLimits::max_error_message`.
    pub max_error_body: usize,
    /// The longest life of one token.
    ///
    /// Default 5 minutes: the default life of a Docker registry token.
    pub token_ttl: Duration,
    /// The deadline for the connection to the registry.
    ///
    /// Default 10 s: the `ArtifactLimits::connect_timeout` default.
    pub connect_timeout: Duration,
    /// The longest time that one body read waits for the next bytes.
    ///
    /// Default 60 s: the `client_body_timeout` default of nginx.
    pub idle_timeout: Duration,
}

impl Default for RegistryLimits {
    fn default() -> Self {
        Self {
            max_manifest_bytes: 4 * 1024 * 1024,
            max_layers: 127,
            max_redirects: 5,
            max_blob_bytes: 4 * 1024 * 1024 * 1024,
            max_error_body: 64 * 1024,
            token_ttl: Duration::from_mins(5),
            connect_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(60),
        }
    }
}

impl RegistryLimits {
    /// A set of limits with every default.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces [`Self::max_manifest_bytes`].
    #[must_use]
    pub fn with_max_manifest_bytes(mut self, value: usize) -> Self {
        self.max_manifest_bytes = value;
        self
    }

    /// Replaces [`Self::max_layers`].
    #[must_use]
    pub fn with_max_layers(mut self, value: usize) -> Self {
        self.max_layers = value;
        self
    }

    /// Replaces [`Self::max_redirects`].
    #[must_use]
    pub fn with_max_redirects(mut self, value: u8) -> Self {
        self.max_redirects = value;
        self
    }

    /// Replaces [`Self::max_blob_bytes`].
    #[must_use]
    pub fn with_max_blob_bytes(mut self, value: u64) -> Self {
        self.max_blob_bytes = value;
        self
    }

    /// Replaces [`Self::max_error_body`].
    #[must_use]
    pub fn with_max_error_body(mut self, value: usize) -> Self {
        self.max_error_body = value;
        self
    }

    /// Replaces [`Self::token_ttl`].
    #[must_use]
    pub fn with_token_ttl(mut self, value: Duration) -> Self {
        self.token_ttl = value;
        self
    }

    /// Replaces [`Self::connect_timeout`].
    #[must_use]
    pub fn with_connect_timeout(mut self, value: Duration) -> Self {
        self.connect_timeout = value;
        self
    }

    /// Replaces [`Self::idle_timeout`].
    #[must_use]
    pub fn with_idle_timeout(mut self, value: Duration) -> Self {
        self.idle_timeout = value;
        self
    }
}

/// The stream of one HTTP exchange.
type ExchangeStream = Pin<Box<dyn Stream<Item = HttpExchange> + Send + 'static>>;

/// One response whose body is still a stream.
pub(crate) struct HttpResponse {
    /// The numeric status code.
    pub(crate) status: usize,
    /// The response headers.
    pub(crate) headers: SimpleHeaders,
    /// The body, after the head.
    pub(crate) stream: ExchangeStream,
}

/// One bearer token that this client holds.
struct TokenEntry {
    /// The registry host.
    registry: String,
    /// The repository that the token is for.
    repository: String,
    /// The token. Its `Debug` form is redacted.
    token: Secret,
    /// The instant when the token stops being used.
    expires_at: Instant,
}

/// The client of one registry pull.
///
/// The client keeps the bearer token of the repository for its own life, so the
/// manifest and the blobs of one import use it without another token request
/// (feature 15, section 6.3). The `Debug` form never shows a token.
pub struct RegistryClient {
    ctx: Ctx,
    fs: Arc<dyn HostFs>,
    cache_dir: PathBuf,
    options: RegistryOptions,
    client: DynNetClient,
    tokens: Mutex<Vec<TokenEntry>>,
}

impl fmt::Debug for RegistryClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RegistryClient")
            .field("cache_dir", &self.cache_dir)
            .field("limits", &self.options.limits)
            .finish_non_exhaustive()
    }
}

impl RegistryClient {
    /// A client over `cache_dir`.
    ///
    /// The client creates the blob cache directory when it writes the first
    /// blob.
    #[must_use]
    pub fn new(
        ctx: Ctx,
        fs: Arc<dyn HostFs>,
        cache_dir: PathBuf,
        options: RegistryOptions,
    ) -> Self {
        let limits = &options.limits;
        let client = HttpClientBuilder::new()
            .connect_timeout(limits.connect_timeout)
            .read_timeout(limits.idle_timeout)
            .max_redirects(limits.max_redirects)
            .preserve_auth_on_redirect(false)
            .refuse_https_downgrade(true)
            .build();
        Self {
            ctx,
            fs,
            cache_dir,
            options,
            client,
            tokens: Mutex::new(Vec::new()),
        }
    }

    /// The options of this client.
    #[must_use]
    pub fn options(&self) -> &RegistryOptions {
        &self.options
    }

    /// The path of the verified blob for `digest`.
    ///
    /// # Errors
    /// [`SandboxError::InvalidSpec`] when the digest is not
    /// `sha256:<64 lower-case hex>`.
    pub fn blob_path(&self, digest: &str) -> SandboxResult<PathBuf> {
        let hex = digest_hex(digest)?;
        Ok(self
            .cache_dir
            .join(BLOB_CACHE)
            .join(BLOB_DIR)
            .join(BLOB_ALGORITHM)
            .join(hex))
    }

    /// Resolves one reference to the manifest for this guest architecture.
    ///
    /// An index is resolved through its entry for `linux/<guest architecture>`.
    /// A manifest is returned as it is.
    ///
    /// # Errors
    /// [`SandboxError::ArtifactHashMismatch`] when a manifest does not match its
    /// digest, [`SandboxError::ImageArchMismatch`] when no entry matches, and
    /// [`SandboxError::InvalidSpec`] for an unsupported media type or a
    /// manifest without a configuration.
    pub async fn resolve_manifest(
        &self,
        reference: &OciReference,
    ) -> SandboxResult<ResolvedManifest> {
        let (body, digest, content_type) = self
            .fetch_manifest_body(reference, reference.reference_text(), reference.digest())
            .await?;
        let document = parse_document(&body)?;
        match document.kind(content_type.as_deref())? {
            DocumentKind::Index => {
                let image = reference.to_string();
                let entry = choose_platform(&document, GuestArch::host(), &image)?.clone();
                let (child_body, child_digest, child_type) = self
                    .fetch_manifest_body(reference, &entry.digest, Some(&entry.digest))
                    .await?;
                let child = parse_document(&child_body)?;
                if child.kind(child_type.as_deref())? != DocumentKind::Manifest {
                    return Err(unsupported_document(&child_body));
                }
                self.check_manifest(&child, reference)?;
                Ok(ResolvedManifest {
                    digest: child_digest,
                    manifest: child,
                })
            }
            DocumentKind::Manifest => {
                self.check_manifest(&document, reference)?;
                Ok(ResolvedManifest {
                    digest,
                    manifest: document,
                })
            }
        }
    }

    /// Reads and parses the configuration blob of `descriptor`.
    ///
    /// # Errors
    /// [`SandboxError::ArtifactHashMismatch`] for a bad blob,
    /// [`SandboxError::ArtifactTooLarge`] when the configuration passes
    /// `max_manifest_bytes`, and [`SandboxError::Io`] for a bad JSON document.
    pub async fn fetch_config(
        &self,
        reference: &OciReference,
        descriptor: &OciDescriptor,
    ) -> SandboxResult<OciImageConfigDocument> {
        let max = self.options.limits.max_manifest_bytes;
        if descriptor.size > u64::try_from(max).unwrap_or(u64::MAX) {
            return Err(SandboxError::ArtifactTooLarge {
                limit: u64::try_from(max).unwrap_or(u64::MAX),
                message: format!(
                    "the configuration blob is {} bytes; the limit is {max}",
                    descriptor.size
                ),
            }
            .into());
        }
        let path = self.fetch_blob(reference, descriptor, &mut |_| {}).await?;
        let body = self.fs.read(&path, max)?;
        let config = serde_json::from_slice(&body).map_err(|error| SandboxError::Io {
            op: "read the image configuration".to_string(),
            message: format!("{}: {error}", path.display()),
        })?;
        Ok(config)
    }

    /// Downloads one blob to the cache and returns its verified path.
    ///
    /// A cached blob with the manifest size is not downloaded again. A partial
    /// blob is removed on every failure (invariant I9).
    ///
    /// # Errors
    /// [`SandboxError::ArtifactHashMismatch`] when the bytes do not match the
    /// digest or the size, [`SandboxError::ArtifactTooLarge`] when the blob
    /// passes `max_blob_bytes`, and [`SandboxError::Io`] for a bad status.
    pub async fn fetch_blob(
        &self,
        reference: &OciReference,
        descriptor: &OciDescriptor,
        progress: &mut (dyn FnMut(u64) + Send),
    ) -> SandboxResult<PathBuf> {
        let path = self.blob_path(&descriptor.digest)?;
        if self.fs.exists(&path) && self.fs.file_size(&path)? == descriptor.size {
            progress(descriptor.size);
            return Ok(path);
        }
        let _ = self.fs.remove_file(&path);
        let parent = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        self.fs.create_dir_all(&parent, 0o755)?;
        let temporary = parent.join(format!(
            "{}.tmp-{}",
            digest_hex(&descriptor.digest)?,
            self.random_name()?
        ));
        if let Err(error) = self
            .download_blob(reference, descriptor, &temporary, progress)
            .await
        {
            let _ = self.fs.remove_file(&temporary);
            return Err(error);
        }
        if let Err(error) = self.fs.rename(&temporary, &path) {
            let _ = self.fs.remove_file(&temporary);
            if self.fs.exists(&path) {
                return Ok(path);
            }
            return Err(error);
        }
        Ok(path)
    }

    /// Streams one blob into `temporary`, checks its size and its digest, and
    /// flushes it.
    async fn download_blob(
        &self,
        reference: &OciReference,
        descriptor: &OciDescriptor,
        temporary: &Path,
        progress: &mut (dyn FnMut(u64) + Send),
    ) -> SandboxResult<()> {
        let url = self.blob_url(reference, &descriptor.digest);
        let mut response = self.send_get(reference, &url, None).await?;
        if response.status != 200 {
            let body = self
                .read_error_body(&mut response.stream, self.options.limits.max_error_body)
                .await;
            return Err(registry_status(response.status, &body));
        }
        let mut writer = self.fs.create_file(temporary)?;
        let mut hasher = Sha256::new();
        let mut done = 0_u64;
        loop {
            let Some(chunk) = self.next_chunk(&mut response.stream).await? else {
                break;
            };
            done = done.saturating_add(u64::try_from(chunk.len()).unwrap_or(u64::MAX));
            if done > descriptor.size {
                return Err(size_mismatch(descriptor.size, done));
            }
            if done > self.options.limits.max_blob_bytes {
                return Err(SandboxError::ArtifactTooLarge {
                    limit: self.options.limits.max_blob_bytes,
                    message: format!("the blob passed the limit after {done} bytes"),
                }
                .into());
            }
            hasher.update(&chunk);
            writer.write_all(&chunk).map_err(|error| SandboxError::Io {
                op: "write a blob".to_string(),
                message: format!("{}: {error}", temporary.display()),
            })?;
            progress(done);
        }
        writer.flush().map_err(|error| SandboxError::Io {
            op: "write a blob".to_string(),
            message: format!("{}: {error}", temporary.display()),
        })?;
        drop(writer);
        if done != descriptor.size {
            return Err(size_mismatch(descriptor.size, done));
        }
        let actual = format!("sha256:{}", hex(&hasher.finalize()));
        if actual != descriptor.digest {
            return Err(SandboxError::ArtifactHashMismatch {
                expected: descriptor.digest.clone(),
                actual,
            }
            .into());
        }
        Ok(())
    }

    /// Reads one manifest body and checks its digest.
    ///
    /// `expected` is the digest that the caller asked for, when it asked for a
    /// digest. A `Docker-Content-Digest` header must equal the computed digest.
    async fn fetch_manifest_body(
        &self,
        reference: &OciReference,
        name: &str,
        expected: Option<&str>,
    ) -> SandboxResult<(Vec<u8>, String, Option<String>)> {
        let url = format!(
            "{}/v2/{}/manifests/{name}",
            self.base_url(reference),
            reference.repository
        );
        let mut response = self
            .send_get(reference, &url, Some(MANIFEST_ACCEPT))
            .await?;
        if response.status != 200 {
            let body = self
                .read_error_body(&mut response.stream, self.options.limits.max_error_body)
                .await;
            return Err(registry_status(response.status, &body));
        }
        let content_type =
            header_first(&response.headers, &SimpleHeader::CONTENT_TYPE).map(str::to_string);
        let body = self
            .read_body_bounded(
                &mut response.stream,
                self.options.limits.max_manifest_bytes,
                "manifest",
            )
            .await?;
        let digest = format!("sha256:{}", hex(&Sha256::digest(&body)));
        if let Some(expected) = expected {
            if digest != expected {
                return Err(SandboxError::ArtifactHashMismatch {
                    expected: expected.to_string(),
                    actual: digest,
                }
                .into());
            }
        }
        if let Some(given) = header_first(&response.headers, &docker_content_digest()) {
            if given != digest {
                return Err(SandboxError::ArtifactHashMismatch {
                    expected: given.to_string(),
                    actual: digest,
                }
                .into());
            }
        }
        Ok((body, digest, content_type))
    }

    /// Checks the fields that every manifest must have.
    fn check_manifest(
        &self,
        manifest: &OciManifestDocument,
        reference: &OciReference,
    ) -> SandboxResult<()> {
        if manifest.config.is_none() {
            return Err(SandboxError::InvalidSpec {
                field: "manifest".to_string(),
                reason: format!("the manifest of {reference} has no configuration"),
            }
            .into());
        }
        if manifest.layers.len() > self.options.limits.max_layers {
            return Err(SandboxError::InvalidSpec {
                field: "manifest".to_string(),
                reason: format!(
                    "the manifest of {reference} has {} layers; the limit is {}",
                    manifest.layers.len(),
                    self.options.limits.max_layers
                ),
            }
            .into());
        }
        Ok(())
    }

    /// Checks the architecture of one configuration against this host.
    ///
    /// # Errors
    /// [`SandboxError::ImageArchMismatch`] when the image is for another guest.
    pub fn check_arch(
        &self,
        config: &OciImageConfigDocument,
        reference: &OciReference,
    ) -> SandboxResult<()> {
        check_image_arch(config, GuestArch::host(), &reference.to_string())
    }

    /// The base address of `reference`, with the scheme that the options give.
    fn base_url(&self, reference: &OciReference) -> String {
        let scheme = if self.options.is_insecure(&reference.registry) {
            "http"
        } else {
            "https"
        };
        format!("{scheme}://{}", reference.registry)
    }

    /// The URL of one blob.
    fn blob_url(&self, reference: &OciReference, digest: &str) -> String {
        format!(
            "{}/v2/{}/blobs/{digest}",
            self.base_url(reference),
            reference.repository
        )
    }

    /// Sends one GET request with the token of this client.
    ///
    /// It sends the request with no `Authorization` header, or with the cached
    /// token, and sends it one more time after a 401 with a challenge. A second
    /// 401, or a 403, is [`SandboxError::Unauthorized`] (feature 15, section
    /// 6.3).
    async fn send_get(
        &self,
        reference: &OciReference,
        url: &str,
        accept: Option<&str>,
    ) -> SandboxResult<HttpResponse> {
        let cached = self.cached_authorization(reference);
        let mut response = self.open_response(url, accept, cached.as_deref()).await?;
        if response.status == 401 {
            let challenge = header_first(&response.headers, &SimpleHeader::WWW_AUTHENTICATE)
                .map(str::to_string);
            // Drain the failed body so the connection goes back to the pool.
            drop(
                self.read_error_body(&mut response.stream, self.options.limits.max_error_body)
                    .await,
            );
            let Some(challenge) = challenge.and_then(|text| parse_www_authenticate(&text)) else {
                return Err(unauthorized());
            };
            let authorization = if challenge.is_bearer() {
                let token = self.fetch_token(reference, &challenge).await?;
                format!("Bearer {}", token.expose())
            } else if challenge.is_basic() {
                let Some(header) = basic_header(self.options.auth_for(&reference.registry)) else {
                    return Err(unauthorized());
                };
                header
            } else {
                return Err(unauthorized());
            };
            response = self
                .open_response(url, accept, Some(&authorization))
                .await?;
            if response.status == 401 || response.status == 403 {
                return Err(unauthorized());
            }
        } else if response.status == 403 {
            return Err(unauthorized());
        }
        Ok(response)
    }

    /// The `Authorization` header of a cached token for this reference.
    fn cached_authorization(&self, reference: &OciReference) -> Option<String> {
        let now = self.ctx.clock().now();
        let tokens = self
            .tokens
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        tokens
            .iter()
            .find(|entry| {
                entry.expires_at > now
                    && entry.registry.eq_ignore_ascii_case(&reference.registry)
                    && entry.repository == reference.repository
            })
            .map(|entry| format!("Bearer {}", entry.token.expose()))
    }

    /// Gets one token from the token service and keeps it.
    async fn fetch_token(
        &self,
        reference: &OciReference,
        challenge: &Challenge,
    ) -> SandboxResult<Secret> {
        let Some(realm) = challenge.param("realm") else {
            return Err(unauthorized());
        };
        check_realm(realm, &self.options.allow_insecure)?;
        let mut url = realm.to_string();
        let mut separator = if url.contains('?') { '&' } else { '?' };
        if let Some(service) = challenge.param("service") {
            url.push(separator);
            url.push_str("service=");
            url.push_str(&query_value(service));
            separator = '&';
        }
        url.push(separator);
        url.push_str("scope=repository:");
        url.push_str(&query_value(&reference.repository));
        url.push_str(":pull");

        let basic = basic_header(self.options.auth_for(&reference.registry));
        let mut response = self.open_response(&url, None, basic.as_deref()).await?;
        if response.status != 200 {
            let body = self
                .read_error_body(&mut response.stream, self.options.limits.max_error_body)
                .await;
            return if response.status == 401 || response.status == 403 {
                Err(unauthorized())
            } else {
                Err(registry_status(response.status, &body))
            };
        }
        let body = self
            .read_body_bounded(
                &mut response.stream,
                self.options.limits.max_error_body,
                "token answer",
            )
            .await?;
        let answer: TokenResponse =
            serde_json::from_slice(&body).map_err(|error| SandboxError::Io {
                op: "read a registry token".to_string(),
                message: error.to_string(),
            })?;
        let expires_in = answer.expires_in;
        let Some(text) = answer.into_token() else {
            return Err(unauthorized());
        };
        let limit = self.options.limits.token_ttl;
        let ttl = expires_in.map_or(limit, |seconds| Duration::from_secs(seconds).min(limit));
        let token = Secret::new(text);
        self.store_token(reference, token.clone(), self.ctx.clock().now() + ttl);
        Ok(token)
    }

    /// Keeps one token for the later calls of the same repository.
    fn store_token(&self, reference: &OciReference, token: Secret, expires_at: Instant) {
        let mut tokens = self
            .tokens
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        tokens.retain(|entry| {
            !(entry.registry.eq_ignore_ascii_case(&reference.registry)
                && entry.repository == reference.repository)
        });
        tokens.push(TokenEntry {
            registry: reference.registry.clone(),
            repository: reference.repository.clone(),
            token,
            expires_at,
        });
    }

    /// Sends one GET request with an optional `Accept` and `Authorization`
    /// header.
    pub(crate) async fn open_response(
        &self,
        url: &str,
        accept: Option<&str>,
        authorization: Option<&str>,
    ) -> SandboxResult<HttpResponse> {
        let mut builder =
            PreparedRequestBuilder::get(url).map_err(|error| transport_error(url, &error))?;
        if let Some(accept) = accept {
            builder = builder.header(SimpleHeader::ACCEPT, accept);
        }
        if let Some(authorization) = authorization {
            builder = builder.header(SimpleHeader::AUTHORIZATION, authorization);
        }
        let request = builder.build();
        let task = self.client.open_exchange(request);
        let driven = execute(task, None).map_err(|error| SandboxError::Io {
            op: "registry".to_string(),
            message: format!("{url}: {error}"),
        })?;
        let mut stream: ExchangeStream = Box::pin(driven.into_next_stream());
        match self.next_item(&mut stream).await? {
            Some(HttpExchange::Head { status, headers }) => Ok(HttpResponse {
                status: status.into_usize(),
                headers,
                stream,
            }),
            Some(HttpExchange::Failed(error)) => Err(SandboxError::Io {
                op: "registry".to_string(),
                message: format!("{url}: {error}"),
            }
            .into()),
            Some(HttpExchange::BodyChunk(_)) => Err(SandboxError::Io {
                op: "registry".to_string(),
                message: format!("{url}: the body started before the response head"),
            }
            .into()),
            None => Err(SandboxError::Io {
                op: "registry".to_string(),
                message: format!("{url}: the answer ended before the response head"),
            }
            .into()),
        }
    }

    /// Waits for the next exchange item with the idle time limit.
    async fn next_item(&self, stream: &mut ExchangeStream) -> SandboxResult<Option<HttpExchange>> {
        let timer = self.ctx.require_env().ok().map(|env| env.timer());
        let Some(timer) = timer else {
            return Ok(stream.next().await);
        };
        let deadline = self.ctx.clock().now() + self.options.limits.idle_timeout;
        match select(stream.next(), timer.sleep_until(self.ctx.clone(), deadline)).await {
            Either::Left((item, _)) => Ok(item),
            Either::Right(_) => Err(SandboxError::Io {
                op: "registry".to_string(),
                message: format!(
                    "the registry sent no bytes for {:?}",
                    self.options.limits.idle_timeout
                ),
            }
            .into()),
        }
    }

    /// The next body chunk, or `None` at the end of the body.
    pub(crate) async fn next_chunk(
        &self,
        stream: &mut ExchangeStream,
    ) -> SandboxResult<Option<Bytes>> {
        match self.next_item(stream).await? {
            None => Ok(None),
            Some(HttpExchange::BodyChunk(chunk)) => Ok(Some(chunk)),
            Some(HttpExchange::Failed(error)) => Err(SandboxError::Io {
                op: "registry".to_string(),
                message: error.to_string(),
            }
            .into()),
            Some(HttpExchange::Head { .. }) => Err(SandboxError::Io {
                op: "registry".to_string(),
                message: "the response sent a second head".to_string(),
            }
            .into()),
        }
    }

    /// Reads one body into memory, bounded by `limit`.
    async fn read_body_bounded(
        &self,
        stream: &mut ExchangeStream,
        limit: usize,
        what: &str,
    ) -> SandboxResult<Vec<u8>> {
        let mut body = Vec::new();
        while let Some(chunk) = self.next_chunk(stream).await? {
            body.extend_from_slice(&chunk);
            if body.len() > limit {
                return Err(SandboxError::ArtifactTooLarge {
                    limit: u64::try_from(limit).unwrap_or(u64::MAX),
                    message: format!("the {what} is larger than {limit} bytes"),
                }
                .into());
            }
        }
        Ok(body)
    }

    /// Reads at most `limit` bytes of a failed body as text.
    async fn read_error_body(&self, stream: &mut ExchangeStream, limit: usize) -> String {
        let mut body = Vec::new();
        while body.len() < limit {
            let Ok(item) = self.next_item(stream).await else {
                break;
            };
            match item {
                Some(HttpExchange::BodyChunk(chunk)) => {
                    let room = limit - body.len();
                    let take = room.min(chunk.len());
                    body.extend_from_slice(&chunk[..take]);
                    if take < chunk.len() {
                        break;
                    }
                }
                Some(HttpExchange::Failed(error)) => {
                    body.extend_from_slice(format!(" ({error})").as_bytes());
                    break;
                }
                Some(HttpExchange::Head { .. }) | None => break,
            }
        }
        String::from_utf8_lossy(&body).into_owned()
    }

    /// A fresh random name for one temporary blob file.
    fn random_name(&self) -> SandboxResult<String> {
        let rng = self.ctx.rng();
        let mut rng = rng.lock().map_err(|_| SandboxError::Io {
            op: "name a blob".to_string(),
            message: "the random number generator lock is poisoned".to_string(),
        })?;
        Ok(format!("{:016x}", rng.next_u64()))
    }
}

/// Parses one manifest document.
fn parse_document(body: &[u8]) -> SandboxResult<OciManifestDocument> {
    serde_json::from_slice(body).map_err(|error| {
        SandboxError::Io {
            op: "read a manifest".to_string(),
            message: error.to_string(),
        }
        .into()
    })
}

/// The error of a document that is not a manifest after an index selected it.
fn unsupported_document(body: &[u8]) -> foundation_errstacks::ErrorTrace<SandboxError> {
    SandboxError::InvalidSpec {
        field: "manifest".to_string(),
        reason: format!(
            "an index entry names no manifest: {} bytes of another document",
            body.len()
        ),
    }
    .into()
}

/// The error of a failed response status.
pub(crate) fn registry_status(
    status: usize,
    body: &str,
) -> foundation_errstacks::ErrorTrace<SandboxError> {
    SandboxError::Io {
        op: "registry".to_string(),
        message: format!("status {status}: {body}"),
    }
    .into()
}

/// The error of a request that the registry did not authorize.
fn unauthorized() -> foundation_errstacks::ErrorTrace<SandboxError> {
    SandboxError::Unauthorized.into()
}

/// The error of a transport failure.
fn transport_error(
    url: &str,
    error: &foundation_netio::shared::http::HttpClientError,
) -> foundation_errstacks::ErrorTrace<SandboxError> {
    SandboxError::Io {
        op: "registry".to_string(),
        message: format!("{url}: {error}"),
    }
    .into()
}

/// The error of a blob whose size differs from the manifest.
fn size_mismatch(expected: u64, actual: u64) -> foundation_errstacks::ErrorTrace<SandboxError> {
    SandboxError::ArtifactHashMismatch {
        expected: format!("{expected} bytes"),
        actual: format!("{actual} bytes"),
    }
    .into()
}

/// The first value of one response header.
fn header_first<'a>(headers: &'a SimpleHeaders, name: &SimpleHeader) -> Option<&'a str> {
    headers
        .get(name)
        .and_then(|values| values.first())
        .map(String::as_str)
}

/// The `Docker-Content-Digest` header.
fn docker_content_digest() -> SimpleHeader {
    SimpleHeader::custom("Docker-Content-Digest")
}

/// The 64 lower-case hexadecimal digits of one `sha256:` digest.
///
/// # Errors
/// [`SandboxError::InvalidSpec`] when the digest is not a `sha256` digest.
pub fn digest_hex(digest: &str) -> SandboxResult<&str> {
    let Some(hex) = digest.strip_prefix("sha256:") else {
        return Err(SandboxError::InvalidSpec {
            field: "digest".to_string(),
            reason: format!("a digest is `sha256:<64 lower-case hex>`: {digest}"),
        }
        .into());
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(SandboxError::InvalidSpec {
            field: "digest".to_string(),
            reason: format!("a digest is `sha256:<64 lower-case hex>`: {digest}"),
        }
        .into());
    }
    Ok(hex)
}

/// The lower-case hexadecimal text of 32 bytes.
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from(DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    text
}
