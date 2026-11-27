//! Envelope encryption with a remote key management service (KMS).
//!
//! WHY: on a cloud host the master key must never sit on the machine. A new
//! data encryption key (DEK) encrypts each private key, and the KMS encrypts
//! the DEK with its master key. The KMS sees only the DEK.
//!
//! WHAT: [`KmsBackend`], [`KmsConfig`], [`EnvelopeSealer`], [`RemoteKmsKek`],
//! [`FakeKms`], and the `HashiCorp` Vault Transit, Amazon Web Services (AWS) KMS
//! and Google Cloud Platform (GCP) Cloud KMS backends.
//!
//! HOW: the sealer uses ChaCha20-Poly1305 with a fresh DEK. The KMS call binds
//! the DEK to the record with the associated data as its context. Credentials
//! come from the platform workload identity, so no long-lived KMS secret sits
//! on the machine.

use std::sync::Arc;

use base64::Engine as _;
use foundation_compact::crypto::chacha::{open_with, seal_with, ChaChaKey};
use foundation_compact::crypto::SecureBytes;
use foundation_context::env::{EntropySourceExt as _, Env};
use zeroize::Zeroizing;

#[cfg(not(target_family = "wasm"))]
use super::kek::{KekSource, KekSourceInfo};
use super::sealing::{SealFuture, SealedBlob};
use super::{KeySealer, KeyStoreError};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;
#[cfg(not(target_family = "wasm"))]
const B64_STD: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// One remote KMS.
pub trait KmsBackend: Send + Sync + 'static {
    /// The name of the KMS key.
    fn key_id(&self) -> &str;

    /// Encrypt `plaintext_dek`. `context` binds the call to one record.
    fn encrypt<'a>(&'a self, plaintext_dek: &'a [u8], context: &'a [u8])
        -> SealFuture<'a, Vec<u8>>;

    /// Decrypt `wrapped_dek` with the same `context`.
    fn decrypt<'a>(
        &'a self,
        wrapped_dek: &'a [u8],
        context: &'a [u8],
    ) -> SealFuture<'a, SecureBytes>;
}

/// How to find a KMS token or secret.
#[derive(Clone)]
pub enum SecretRef {
    /// A file, for example a Docker secret.
    File(std::path::PathBuf),
    /// An environment variable.
    Environment(String),
    /// A value that the host already read.
    Inline(SecureBytes),
}

impl std::fmt::Debug for SecretRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File(path) => f.debug_tuple("SecretRef::File").field(path).finish(),
            Self::Environment(name) => f.debug_tuple("SecretRef::Environment").field(name).finish(),
            Self::Inline(_) => f.write_str("SecretRef::Inline(<redacted>)"),
        }
    }
}

impl SecretRef {
    /// Read the secret.
    ///
    /// # Errors
    /// [`KeyStoreError::KekUnavailable`] when the file or the variable is
    /// absent.
    pub fn resolve(&self) -> Result<SecureBytes, KeyStoreError> {
        match self {
            #[cfg(not(target_family = "wasm"))]
            Self::File(path) => std::fs::read(path).map(SecureBytes::new).map_err(|e| {
                KeyStoreError::KekUnavailable(format!("secret file {}: {e}", path.display()))
            }),
            #[cfg(target_family = "wasm")]
            Self::File(_) => Err(KeyStoreError::KekUnavailable(
                "a file secret is not available on wasm".to_string(),
            )),
            #[cfg(not(target_family = "wasm"))]
            Self::Environment(name) => std::env::var(name)
                .map(|value| SecureBytes::new(value.into_bytes()))
                .map_err(|_| {
                    KeyStoreError::KekUnavailable(format!("environment variable {name} is not set"))
                }),
            #[cfg(target_family = "wasm")]
            Self::Environment(_) => Err(KeyStoreError::KekUnavailable(
                "an environment secret is not available on wasm".to_string(),
            )),
            Self::Inline(value) => Ok(value.clone()),
        }
    }
}

/// One remote KMS, as the host configuration names it.
#[derive(Debug, Clone)]
pub enum KmsConfig {
    /// `HashiCorp` Vault Transit engine: `POST /v1/transit/encrypt/<key>`. The
    /// key must be derived, because the design binds each call to its record
    /// with the context.
    VaultTransit {
        /// The Vault address, for example `https://vault.example:8200`.
        address: String,
        /// The Transit key name.
        key_name: String,
        /// The Vault token.
        token_source: SecretRef,
    },
    /// AWS KMS `Encrypt` / `Decrypt`, signed with the instance role from the
    /// instance metadata service version 2 (`IMDSv2`).
    AwsKms {
        /// The AWS region.
        region: String,
        /// The KMS key ID or alias.
        key_id: String,
    },
    /// GCP Cloud KMS `encrypt` / `decrypt`, with the workload's service
    /// account from the metadata server.
    GcpKms {
        /// The full key resource name.
        key_name: String,
    },
}

/// A sealer that encrypts each blob with a fresh data encryption key (DEK),
/// and asks the KMS to wrap the DEK.
pub struct EnvelopeSealer {
    kek_id: String,
    backend: Arc<dyn KmsBackend>,
    env: Env,
}

impl std::fmt::Debug for EnvelopeSealer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnvelopeSealer")
            .field("kek_id", &self.kek_id)
            .finish_non_exhaustive()
    }
}

impl EnvelopeSealer {
    /// A sealer over `backend`.
    #[must_use]
    pub fn new(kek_id: impl Into<String>, backend: Arc<dyn KmsBackend>, env: Env) -> Self {
        Self {
            kek_id: kek_id.into(),
            backend,
            env,
        }
    }

    fn random<const N: usize>(&self) -> Result<[u8; N], KeyStoreError> {
        self.env
            .entropy()
            .fill_new::<N>()
            .map_err(|e| KeyStoreError::Seal(format!("entropy source: {e}")))
    }
}

impl KeySealer for EnvelopeSealer {
    fn kek_id(&self) -> &str {
        &self.kek_id
    }

    fn seal<'a>(&'a self, aad: &'a [u8], plaintext: &'a [u8]) -> SealFuture<'a, SealedBlob> {
        Box::pin(async move {
            let dek = Zeroizing::new(self.random::<32>()?);
            let nonce = self.random::<12>()?;
            let sealed = seal_with(&ChaChaKey::from_bytes(*dek), nonce, aad, plaintext)
                .map_err(KeyStoreError::Seal)?;
            let wrapped = self.backend.encrypt(&*dek, aad).await?;
            Ok(SealedBlob {
                v: 1,
                alg: "kms-envelope+chacha20poly1305".to_string(),
                kek_id: self.kek_id.clone(),
                nonce_b64url: B64.encode(nonce),
                ciphertext_b64url: B64.encode(&sealed[12..]),
                salt_b64url: None,
                argon2: None,
                wrapped_dek_b64url: Some(B64.encode(&wrapped)),
            })
        })
    }

    fn open<'a>(&'a self, aad: &'a [u8], sealed: &'a SealedBlob) -> SealFuture<'a, SecureBytes> {
        Box::pin(async move {
            if sealed.alg != "kms-envelope+chacha20poly1305" || sealed.kek_id != self.kek_id {
                return Err(KeyStoreError::Seal(
                    "the blob does not belong to this KMS key".to_string(),
                ));
            }
            let wrapped = sealed
                .wrapped_dek_b64url
                .as_deref()
                .ok_or_else(|| KeyStoreError::Seal("the blob has no wrapped DEK".to_string()))?;
            let wrapped = B64
                .decode(wrapped.trim())
                .map_err(|e| KeyStoreError::Seal(format!("wrapped DEK: {e}")))?;
            let dek = self.backend.decrypt(&wrapped, aad).await?;
            let dek: [u8; 32] = dek.as_slice().try_into().map_err(|_| {
                KeyStoreError::Seal("the KMS returned a DEK that is not 32 bytes".to_string())
            })?;
            let dek = Zeroizing::new(dek);
            let nonce: [u8; 12] = B64
                .decode(sealed.nonce_b64url.trim())
                .map_err(|e| KeyStoreError::Seal(format!("nonce: {e}")))?
                .try_into()
                .map_err(|_| KeyStoreError::Seal("the nonce is not 12 bytes".to_string()))?;
            let ciphertext = B64
                .decode(sealed.ciphertext_b64url.trim())
                .map_err(|e| KeyStoreError::Seal(format!("ciphertext: {e}")))?;
            open_with(&ChaChaKey::from_bytes(*dek), &nonce, aad, &ciphertext)
                .map_err(KeyStoreError::Seal)
        })
    }
}

/// A remote KMS as a [`KekSource`].
#[cfg(not(target_family = "wasm"))]
pub struct RemoteKmsKek {
    /// The KMS configuration.
    pub config: KmsConfig,
    /// The HTTP client.
    pub http: foundation_netio::DynNetClient,
    /// The KEK ID.
    pub kek_id: String,
    /// The clock for the AWS signature timestamp.
    pub clock: std::sync::Arc<dyn foundation_context::Clock>,
}

#[cfg(not(target_family = "wasm"))]
impl std::fmt::Debug for RemoteKmsKek {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteKmsKek")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

#[cfg(not(target_family = "wasm"))]
impl RemoteKmsKek {
    /// A remote KMS source with the default HTTP client.
    ///
    /// # Errors
    /// [`KeyStoreError::KekUnavailable`] when the backend cannot be built.
    pub fn new(
        config: KmsConfig,
        kek_id: impl Into<String>,
        clock: std::sync::Arc<dyn foundation_context::Clock>,
    ) -> Result<Self, KeyStoreError> {
        Ok(Self {
            config,
            http: foundation_netio::HttpClientBuilder::new().build(),
            kek_id: kek_id.into(),
            clock,
        })
    }
}

#[cfg(not(target_family = "wasm"))]
impl KekSource for RemoteKmsKek {
    fn describe(&self) -> KekSourceInfo {
        KekSourceInfo {
            kind: "remote-kms",
            location: match &self.config {
                KmsConfig::VaultTransit {
                    address, key_name, ..
                } => {
                    format!("{address} transit/{key_name}")
                }
                KmsConfig::AwsKms { region, key_id } => format!("aws-kms {region} {key_id}"),
                KmsConfig::GcpKms { key_name } => format!("gcp-kms {key_name}"),
            },
            kek_id: self.kek_id.clone(),
            // The master key stays in the KMS.
            key_leaves_process: true,
        }
    }

    fn load<'a>(&'a self, env: &'a Env) -> SealFuture<'a, Arc<dyn KeySealer>> {
        Box::pin(async move {
            let backend = build_backend(&self.config, self.http.clone(), self.clock.clone())?;
            Ok(Arc::new(EnvelopeSealer::new(
                self.kek_id.clone(),
                backend,
                env.clone(),
            )) as Arc<dyn KeySealer>)
        })
    }
}

/// Build the backend that `config` names.
///
/// # Errors
/// [`KeyStoreError::KekUnavailable`] when a token or a credential is missing.
#[cfg(not(target_family = "wasm"))]
pub fn build_backend(
    config: &KmsConfig,
    http: foundation_netio::DynNetClient,
    clock: std::sync::Arc<dyn foundation_context::Clock>,
) -> Result<Arc<dyn KmsBackend>, KeyStoreError> {
    match config {
        KmsConfig::VaultTransit {
            address,
            key_name,
            token_source,
        } => Ok(Arc::new(VaultTransitBackend::new(
            address.clone(),
            key_name.clone(),
            token_source.resolve()?,
            http,
        ))),
        KmsConfig::AwsKms { region, key_id } => Ok(Arc::new(AwsKmsBackend::new(
            region.clone(),
            key_id.clone(),
            http,
            clock,
        ))),
        KmsConfig::GcpKms { key_name } => Ok(Arc::new(GcpKmsBackend::new(key_name.clone(), http))),
    }
}

/// A fake KMS for a test or a deterministic simulation run.
#[cfg(feature = "testing")]
pub struct FakeKms {
    key_id: String,
    key: ChaChaKey,
}

#[cfg(feature = "testing")]
impl FakeKms {
    /// A fake KMS with a fixed master key.
    #[must_use]
    pub fn new(key_id: impl Into<String>, master_key: [u8; 32]) -> Self {
        Self {
            key_id: key_id.into(),
            key: ChaChaKey::from_bytes(master_key),
        }
    }
}

#[cfg(feature = "testing")]
impl KmsBackend for FakeKms {
    fn key_id(&self) -> &str {
        &self.key_id
    }

    fn encrypt<'a>(
        &'a self,
        plaintext_dek: &'a [u8],
        context: &'a [u8],
    ) -> SealFuture<'a, Vec<u8>> {
        Box::pin(async move {
            let nonce = foundation_compact::crypto::generate_nonce::<12>();
            seal_with(&self.key, nonce, context, plaintext_dek).map_err(KeyStoreError::Seal)
        })
    }

    fn decrypt<'a>(
        &'a self,
        wrapped_dek: &'a [u8],
        context: &'a [u8],
    ) -> SealFuture<'a, SecureBytes> {
        Box::pin(async move {
            if wrapped_dek.len() < 12 {
                return Err(KeyStoreError::Seal("wrapped DEK is too short".to_string()));
            }
            let nonce: [u8; 12] = wrapped_dek[..12]
                .try_into()
                .map_err(|_| KeyStoreError::Seal("wrapped DEK nonce".to_string()))?;
            open_with(&self.key, &nonce, context, &wrapped_dek[12..]).map_err(KeyStoreError::Seal)
        })
    }
}

// ===========================================================================
// HTTP backends (native)
// ===========================================================================

#[cfg(not(target_family = "wasm"))]
use foundation_netio::DynNetClient;

/// The HTTP verb of a KMS or metadata request.
#[cfg(not(target_family = "wasm"))]
#[derive(Clone, Copy)]
enum HttpVerb {
    Get,
    Post,
    Put,
}

/// Send a JSON request and return the response body of a 2xx answer.
///
/// The response of a KMS is small JSON that the caller parses and acts on, so
/// it is read whole. An error answer keeps at most the documented bound.
#[cfg(not(target_family = "wasm"))]
async fn request_json(
    verb: HttpVerb,
    http: &DynNetClient,
    url: &str,
    headers: &[(String, String)],
    body: &str,
) -> Result<String, KeyStoreError> {
    let builder = match verb {
        HttpVerb::Get => foundation_netio::PreparedRequestBuilder::get(url),
        HttpVerb::Post => foundation_netio::PreparedRequestBuilder::post(url),
        HttpVerb::Put => foundation_netio::PreparedRequestBuilder::put(url),
    }
    .map_err(|e| KeyStoreError::Storage(format!("KMS request URL: {e}")))?;
    let mut builder = builder;
    // AWS KMS signs the exact `Content-Type` value and needs
    // `application/x-amz-json-1.1`. Adding the default a second time sends two
    // values, and KMS then answers `UnknownOperationException`. Use the
    // caller's value when it gives one.
    let has_content_type = headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("content-type"));
    if !has_content_type {
        builder = builder.header(
            foundation_netio::shared::http::SimpleHeader::CONTENT_TYPE,
            "application/json",
        );
    }
    for (name, value) in headers {
        builder = builder.header(name.clone(), value.clone());
    }
    if !body.is_empty() {
        builder = builder.body_text(body);
    }
    let response = http
        .send_async(builder.build())
        .await
        .map_err(|e| KeyStoreError::Storage(format!("KMS request: {e}")))?;
    let status: usize = response.get_status().into();
    if (200..=299).contains(&status) {
        crate::shared::body::read_body_text(response.take_async_body())
            .await
            .map_err(KeyStoreError::Storage)
    } else {
        let text = crate::shared::body::read_error_text(
            response.take_async_body(),
            crate::shared::body::DEFAULT_MAX_ERROR_BODY,
        )
        .await;
        Err(KeyStoreError::Storage(format!(
            "the KMS answered HTTP {status}: {text}"
        )))
    }
}

/// `HashiCorp` Vault Transit.
#[cfg(not(target_family = "wasm"))]
pub struct VaultTransitBackend {
    address: String,
    key_name: String,
    token: SecureBytes,
    http: DynNetClient,
}

#[cfg(not(target_family = "wasm"))]
impl std::fmt::Debug for VaultTransitBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VaultTransitBackend")
            .field("address", &self.address)
            .field("key_name", &self.key_name)
            .finish_non_exhaustive()
    }
}

#[cfg(not(target_family = "wasm"))]
impl VaultTransitBackend {
    /// A Vault Transit backend. The key must be derived.
    #[must_use]
    pub fn new(address: String, key_name: String, token: SecureBytes, http: DynNetClient) -> Self {
        Self {
            address,
            key_name,
            token,
            http,
        }
    }

    fn token_text(&self) -> Result<String, KeyStoreError> {
        String::from_utf8(self.token.to_vec())
            .map_err(|_| KeyStoreError::KekUnavailable("the Vault token is not UTF-8".to_string()))
    }
}

#[cfg(not(target_family = "wasm"))]
impl KmsBackend for VaultTransitBackend {
    fn key_id(&self) -> &str {
        &self.key_name
    }

    fn encrypt<'a>(
        &'a self,
        plaintext_dek: &'a [u8],
        context: &'a [u8],
    ) -> SealFuture<'a, Vec<u8>> {
        Box::pin(async move {
            let url = format!("{}/v1/transit/encrypt/{}", self.address, self.key_name);
            let body = serde_json::json!({
                "plaintext": B64_STD.encode(plaintext_dek),
                "context": B64_STD.encode(context),
            })
            .to_string();
            let token = self.token_text()?;
            let response = request_json(
                HttpVerb::Post,
                &self.http,
                &url,
                &[("X-Vault-Token".to_string(), token)],
                &body,
            )
            .await?;
            let value: serde_json::Value = serde_json::from_str(&response)
                .map_err(|e| KeyStoreError::Storage(format!("Vault answer: {e}")))?;
            let ciphertext = value
                .get("data")
                .and_then(|data| data.get("ciphertext"))
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    KeyStoreError::Storage("Vault answer has no ciphertext".to_string())
                })?;
            Ok(ciphertext.as_bytes().to_vec())
        })
    }

    fn decrypt<'a>(
        &'a self,
        wrapped_dek: &'a [u8],
        context: &'a [u8],
    ) -> SealFuture<'a, SecureBytes> {
        Box::pin(async move {
            let url = format!("{}/v1/transit/decrypt/{}", self.address, self.key_name);
            let ciphertext = String::from_utf8(wrapped_dek.to_vec())
                .map_err(|_| KeyStoreError::Seal("the wrapped DEK is not UTF-8".to_string()))?;
            let body = serde_json::json!({
                "ciphertext": ciphertext,
                "context": B64_STD.encode(context),
            })
            .to_string();
            let token = self.token_text()?;
            let response = request_json(
                HttpVerb::Post,
                &self.http,
                &url,
                &[("X-Vault-Token".to_string(), token)],
                &body,
            )
            .await?;
            let value: serde_json::Value = serde_json::from_str(&response)
                .map_err(|e| KeyStoreError::Storage(format!("Vault answer: {e}")))?;
            let plaintext = value
                .get("data")
                .and_then(|data| data.get("plaintext"))
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    KeyStoreError::Storage("Vault answer has no plaintext".to_string())
                })?;
            let bytes = B64_STD
                .decode(plaintext)
                .map_err(|e| KeyStoreError::Seal(format!("Vault plaintext: {e}")))?;
            Ok(SecureBytes::new(bytes))
        })
    }
}

/// AWS KMS, signed with `SigV4`.
#[cfg(not(target_family = "wasm"))]
pub struct AwsKmsBackend {
    region: String,
    key_id: String,
    http: DynNetClient,
    credentials: Option<AwsCredentials>,
    clock: std::sync::Arc<dyn foundation_context::Clock>,
    endpoint: Option<String>,
}

#[cfg(not(target_family = "wasm"))]
impl std::fmt::Debug for AwsKmsBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AwsKmsBackend")
            .field("region", &self.region)
            .field("key_id", &self.key_id)
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

#[cfg(not(target_family = "wasm"))]
#[derive(Clone)]
struct AwsCredentials {
    access_key: String,
    secret_key: String,
    session_token: Option<String>,
}

#[cfg(not(target_family = "wasm"))]
impl AwsKmsBackend {
    /// An AWS KMS backend. It takes credentials from the environment or from
    /// `IMDSv2` at the first call.
    #[must_use]
    pub fn new(
        region: String,
        key_id: String,
        http: DynNetClient,
        clock: std::sync::Arc<dyn foundation_context::Clock>,
    ) -> Self {
        Self {
            region,
            key_id,
            http,
            credentials: None,
            clock,
            endpoint: None,
        }
    }

    /// Set the KMS endpoint, for a local emulator.
    ///
    /// The default is the regional endpoint, `https://kms.<region>.amazonaws.com`.
    /// When no endpoint is set here, the `AWS_ENDPOINT_URL_KMS` variable is
    /// read, which is the convention of the AWS SDK. The `SigV4` canonical host
    /// and the `Host` header use the endpoint host and port.
    ///
    /// The endpoint must be an origin, for example `http://localhost:5000`.
    #[must_use]
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Some(endpoint.into());
        self
    }

    /// The endpoint text for this backend.
    ///
    /// Order: the endpoint that the builder set, the `AWS_ENDPOINT_URL_KMS`
    /// variable, then the regional endpoint. An empty value counts as absent.
    fn endpoint_text(&self) -> String {
        let configured = self
            .endpoint
            .clone()
            .or_else(|| std::env::var("AWS_ENDPOINT_URL_KMS").ok())
            .unwrap_or_default();
        let configured = configured.trim();
        if configured.is_empty() {
            format!("https://kms.{}.amazonaws.com", self.region)
        } else {
            configured.to_string()
        }
    }

    /// Split the endpoint into its request URL and `SigV4` canonical host.
    ///
    /// The canonical host carries the port when the endpoint names one, because
    /// the HTTP client writes the same text into the `Host` header.
    fn endpoint_parts(&self) -> Result<(String, String), KeyStoreError> {
        let endpoint = self.endpoint_text();
        let uri = foundation_core::url::Uri::parse(&endpoint).map_err(|e| {
            KeyStoreError::KekUnavailable(format!("AWS KMS endpoint {endpoint}: {e}"))
        })?;
        let host_name = uri.host_str().ok_or_else(|| {
            KeyStoreError::KekUnavailable(format!("AWS KMS endpoint {endpoint} has no host"))
        })?;
        let path = uri.path();
        if !path.is_empty() && path != "/" {
            return Err(KeyStoreError::KekUnavailable(format!(
                "AWS KMS endpoint {endpoint} must be an origin with no path"
            )));
        }
        let host = match uri.port() {
            Some(port) => format!("{host_name}:{port}"),
            None => host_name,
        };
        let url = format!("{}/", endpoint.trim_end_matches('/'));
        Ok((url, host))
    }

    /// Credentials from the environment, then from `IMDSv2`.
    async fn credentials(&self) -> Result<AwsCredentials, KeyStoreError> {
        if let Some(credentials) = &self.credentials {
            return Ok(credentials.clone());
        }
        if let (Ok(access_key), Ok(secret_key)) = (
            std::env::var("AWS_ACCESS_KEY_ID"),
            std::env::var("AWS_SECRET_ACCESS_KEY"),
        ) {
            return Ok(AwsCredentials {
                access_key,
                secret_key,
                session_token: std::env::var("AWS_SESSION_TOKEN").ok(),
            });
        }
        let token = self.imds_token().await?;
        let role = request_json(
            HttpVerb::Get,
            &self.http,
            "http://169.254.169.254/latest/meta-data/iam/security-credentials/",
            &[("X-aws-ec2-metadata-token".to_string(), token.clone())],
            "",
        )
        .await?;
        let role = role.trim().to_string();
        let url =
            format!("http://169.254.169.254/latest/meta-data/iam/security-credentials/{role}");
        let body = request_json(
            HttpVerb::Get,
            &self.http,
            &url,
            &[("X-aws-ec2-metadata-token".to_string(), token)],
            "",
        )
        .await?;
        let value: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| KeyStoreError::KekUnavailable(format!("IMDSv2 answer: {e}")))?;
        let field = |name: &str| {
            value
                .get(name)
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| {
                    KeyStoreError::KekUnavailable(format!("IMDSv2 answer has no {name}"))
                })
        };
        Ok(AwsCredentials {
            access_key: field("AccessKeyId")?,
            secret_key: field("SecretAccessKey")?,
            session_token: field("Token").ok(),
        })
    }

    async fn imds_token(&self) -> Result<String, KeyStoreError> {
        // The metadata service needs a PUT with a token time-to-live.
        request_json(
            HttpVerb::Put,
            &self.http,
            "http://169.254.169.254/latest/api/token",
            &[(
                "X-aws-ec2-metadata-token-ttl-seconds".to_string(),
                "60".to_string(),
            )],
            "",
        )
        .await
    }

    async fn call(&self, target: &str, body: &str) -> Result<String, KeyStoreError> {
        use sha2::Digest as _;
        let credentials = self.credentials().await?;
        let (url, host) = self.endpoint_parts()?;
        let now: chrono::DateTime<chrono::Utc> = self.clock.wall_now().into();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date = now.format("%Y%m%d").to_string();
        let content_type = "application/x-amz-json-1.1";
        // The signed headers must be in sorted order, and the session token
        // sits between x-amz-date and x-amz-target.
        let mut signed = vec![
            ("content-type", content_type.to_string()),
            ("host", host.clone()),
            ("x-amz-date", amz_date.clone()),
        ];
        if let Some(token) = &credentials.session_token {
            signed.push(("x-amz-security-token", token.clone()));
        }
        signed.push(("x-amz-target", target.to_string()));
        let mut canonical_headers = String::new();
        for (name, value) in &signed {
            let _ = std::fmt::Write::write_fmt(
                &mut canonical_headers,
                format_args!("{name}:{value}\n"),
            );
        }
        let signed_headers = signed
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>()
            .join(";");
        let payload_hash = hex_lower(&sha2::Sha256::digest(body.as_bytes()));
        let canonical_request =
            format!("POST\n/\n\n{canonical_headers}\n{signed_headers}\n{payload_hash}");
        let scope = format!("{date}/{}/kms/aws4_request", self.region);
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
            hex_lower(&sha2::Sha256::digest(canonical_request.as_bytes()))
        );
        let signature = aws_signature(
            &credentials.secret_key,
            &date,
            &self.region,
            &string_to_sign,
        );
        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
            credentials.access_key
        );
        let mut headers = vec![
            ("Content-Type".to_string(), content_type.to_string()),
            ("X-Amz-Date".to_string(), amz_date),
            ("X-Amz-Target".to_string(), target.to_string()),
            ("Authorization".to_string(), authorization),
        ];
        if let Some(token) = &credentials.session_token {
            headers.push(("X-Amz-Security-Token".to_string(), token.clone()));
        }
        request_json(HttpVerb::Post, &self.http, &url, &headers, body).await
    }
}

#[cfg(not(target_family = "wasm"))]
impl KmsBackend for AwsKmsBackend {
    fn key_id(&self) -> &str {
        &self.key_id
    }

    fn encrypt<'a>(
        &'a self,
        plaintext_dek: &'a [u8],
        context: &'a [u8],
    ) -> SealFuture<'a, Vec<u8>> {
        Box::pin(async move {
            let body = serde_json::json!({
                "KeyId": self.key_id,
                "Plaintext": B64_STD.encode(plaintext_dek),
                "EncryptionContext": { "ewe-aad": B64.encode(context) },
            })
            .to_string();
            let response = self.call("TrentService.Encrypt", &body).await?;
            let value: serde_json::Value = serde_json::from_str(&response)
                .map_err(|e| KeyStoreError::Storage(format!("AWS KMS answer: {e}")))?;
            let blob = value
                .get("CiphertextBlob")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| KeyStoreError::Storage("AWS KMS answer has no blob".to_string()))?;
            B64_STD
                .decode(blob)
                .map_err(|e| KeyStoreError::Storage(format!("AWS KMS blob: {e}")))
        })
    }

    fn decrypt<'a>(
        &'a self,
        wrapped_dek: &'a [u8],
        context: &'a [u8],
    ) -> SealFuture<'a, SecureBytes> {
        Box::pin(async move {
            // `KeyId` binds the call to this backend's key. Without it, AWS
            // KMS decrypts with the key named inside the ciphertext blob, so a
            // blob sealed under another key would open. With it, a blob from
            // another key fails with `IncorrectKeyException`.
            let body = serde_json::json!({
                "KeyId": self.key_id,
                "CiphertextBlob": B64_STD.encode(wrapped_dek),
                "EncryptionContext": { "ewe-aad": B64.encode(context) },
            })
            .to_string();
            let response = self.call("TrentService.Decrypt", &body).await?;
            let value: serde_json::Value = serde_json::from_str(&response)
                .map_err(|e| KeyStoreError::Storage(format!("AWS KMS answer: {e}")))?;
            let plaintext = value
                .get("Plaintext")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    KeyStoreError::Storage("AWS KMS answer has no plaintext".to_string())
                })?;
            let bytes = B64_STD
                .decode(plaintext)
                .map_err(|e| KeyStoreError::Storage(format!("AWS KMS plaintext: {e}")))?;
            Ok(SecureBytes::new(bytes))
        })
    }
}

/// GCP Cloud KMS.
#[cfg(not(target_family = "wasm"))]
pub struct GcpKmsBackend {
    key_name: String,
    http: DynNetClient,
}

#[cfg(not(target_family = "wasm"))]
impl std::fmt::Debug for GcpKmsBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GcpKmsBackend")
            .field("key_name", &self.key_name)
            .finish_non_exhaustive()
    }
}

#[cfg(not(target_family = "wasm"))]
impl GcpKmsBackend {
    /// A GCP Cloud KMS backend. It takes the access token from the metadata
    /// server at each call.
    #[must_use]
    pub fn new(key_name: String, http: DynNetClient) -> Self {
        Self { key_name, http }
    }

    /// The OAuth 2.0 access token for Cloud KMS.
    ///
    /// On a Google Compute Engine (GCE) instance or a Google Kubernetes Engine
    /// (GKE) pod the token comes from the metadata server, and the workload
    /// identity supplies it. On any other host, such as a developer machine or
    /// a CI runner with a service-account key, set `GCP_ACCESS_TOKEN` to the
    /// value of `gcloud auth print-access-token`. The environment source comes
    /// first, so a test or a short-lived job needs no metadata server. The
    /// variable is a secret: never print it or write it to a file.
    async fn access_token(&self) -> Result<String, KeyStoreError> {
        if let Ok(token) = std::env::var("GCP_ACCESS_TOKEN") {
            if !token.trim().is_empty() {
                return Ok(token);
            }
        }
        let body = request_json(
            HttpVerb::Get,
            &self.http,
            "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token",
            &[("Metadata-Flavor".to_string(), "Google".to_string())],
            "",
        )
        .await?;
        let value: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| KeyStoreError::KekUnavailable(format!("GCP metadata answer: {e}")))?;
        value
            .get("access_token")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| {
                KeyStoreError::KekUnavailable("GCP metadata answer has no access token".to_string())
            })
    }

    async fn call(&self, action: &str, body: &str) -> Result<String, KeyStoreError> {
        let token = self.access_token().await?;
        let url = format!(
            "https://cloudkms.googleapis.com/v1/{}:{action}",
            self.key_name
        );
        request_json(
            HttpVerb::Post,
            &self.http,
            &url,
            &[("Authorization".to_string(), format!("Bearer {token}"))],
            body,
        )
        .await
    }
}

#[cfg(not(target_family = "wasm"))]
impl KmsBackend for GcpKmsBackend {
    fn key_id(&self) -> &str {
        &self.key_name
    }

    fn encrypt<'a>(
        &'a self,
        plaintext_dek: &'a [u8],
        context: &'a [u8],
    ) -> SealFuture<'a, Vec<u8>> {
        Box::pin(async move {
            let body = serde_json::json!({
                "plaintext": B64_STD.encode(plaintext_dek),
                "additionalAuthenticatedData": B64_STD.encode(context),
            })
            .to_string();
            let response = self.call("encrypt", &body).await?;
            let value: serde_json::Value = serde_json::from_str(&response)
                .map_err(|e| KeyStoreError::Storage(format!("GCP KMS answer: {e}")))?;
            let ciphertext = value
                .get("ciphertext")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    KeyStoreError::Storage("GCP KMS answer has no ciphertext".to_string())
                })?;
            B64_STD
                .decode(ciphertext)
                .map_err(|e| KeyStoreError::Storage(format!("GCP KMS ciphertext: {e}")))
        })
    }

    fn decrypt<'a>(
        &'a self,
        wrapped_dek: &'a [u8],
        context: &'a [u8],
    ) -> SealFuture<'a, SecureBytes> {
        Box::pin(async move {
            let body = serde_json::json!({
                "ciphertext": B64_STD.encode(wrapped_dek),
                "additionalAuthenticatedData": B64_STD.encode(context),
            })
            .to_string();
            let response = self.call("decrypt", &body).await?;
            let value: serde_json::Value = serde_json::from_str(&response)
                .map_err(|e| KeyStoreError::Storage(format!("GCP KMS answer: {e}")))?;
            let plaintext = value
                .get("plaintext")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    KeyStoreError::Storage("GCP KMS answer has no plaintext".to_string())
                })?;
            let bytes = B64_STD
                .decode(plaintext)
                .map_err(|e| KeyStoreError::Storage(format!("GCP KMS plaintext: {e}")))?;
            Ok(SecureBytes::new(bytes))
        })
    }
}

/// Lowercase hexadecimal text.
#[cfg(not(target_family = "wasm"))]
fn hex_lower(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

/// The `SigV4` signature of a string-to-sign.
#[cfg(not(target_family = "wasm"))]
fn aws_signature(secret_key: &str, date: &str, region: &str, string_to_sign: &str) -> String {
    use hmac::{Hmac, KeyInit as _, Mac as _};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;
    let hmac = |key: &[u8], data: &[u8]| -> Vec<u8> {
        let mut mac = HmacSha256::new_from_slice(key).expect("HMAC takes any key length");
        mac.update(data);
        mac.finalize().into_bytes().to_vec()
    };
    let k_date = hmac(format!("AWS4{secret_key}").as_bytes(), date.as_bytes());
    let k_region = hmac(&k_date, region.as_bytes());
    let k_service = hmac(&k_region, b"kms");
    let k_signing = hmac(&k_service, b"aws4_request");
    hex_lower(&hmac(&k_signing, string_to_sign.as_bytes()))
}
