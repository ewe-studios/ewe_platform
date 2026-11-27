//! The Cloud KMS gRPC backend.

use std::sync::Arc;
use std::time::Duration;

use foundation_auth::keystore::{KeyStoreError, KmsBackend, SealFuture, SecretRef};
use foundation_compact::crypto::SecureBytes;
use foundation_connectrpc::{
    Client, ClientOptions, H2Transport, ProcedureCodecs, ProtoCodec, Request, RpcCtx, Transport,
};
use proto::google::cloud::kms::v1::{
    crypto_key::CryptoKeyPurpose, crypto_key_version, CryptoKey, CryptoKeyVersion, DecryptRequest,
    DecryptResponse, DestroyCryptoKeyVersionRequest, EncryptRequest, EncryptResponse,
    GetCryptoKeyRequest, GetCryptoKeyVersionRequest, KeyRing,
};

use crate::proto;

/// The full name of the Cloud KMS service. It forms the gRPC method path.
const SERVICE: &str = "google.cloud.kms.v1.KeyManagementService";

/// The settings of [`GcpKmsGrpcBackend`].
///
/// `project`, `location` and `key_ring` name the parent resource of the key
/// management calls. This crate gives them no default: a missing value is a
/// [`KeyStoreError::Config`], never a silent guess.
#[derive(Debug, Clone)]
pub struct GcpKmsGrpcConfig {
    endpoint: String,
    timeout: Duration,
    project: Option<String>,
    location: Option<String>,
    key_ring: Option<String>,
    token: Option<SecretRef>,
}

impl GcpKmsGrpcConfig {
    /// The deadline of one gRPC call. 30 s is the budget the live REST tests
    /// allow a call, so both paths wait the same time.
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

    /// The settings for the Cloud KMS gRPC endpoint `endpoint`.
    ///
    /// The endpoint is an IP literal with its port, for example
    /// `http://127.0.0.1:8091`. The h2c transport does no name lookup and no
    /// TLS, so a name and an `https` scheme are configuration errors.
    #[must_use]
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            timeout: Self::DEFAULT_TIMEOUT,
            project: None,
            location: None,
            key_ring: None,
            token: None,
        }
    }

    /// Set the deadline of one call. Default [`Self::DEFAULT_TIMEOUT`].
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the Google Cloud project that holds the key ring.
    #[must_use]
    pub fn with_project(mut self, project: impl Into<String>) -> Self {
        self.project = Some(project.into());
        self
    }

    /// Set the location of the key ring, for example `us-central1`.
    #[must_use]
    pub fn with_location(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }

    /// Set the key ring that the management calls work in.
    #[must_use]
    pub fn with_key_ring(mut self, key_ring: impl Into<String>) -> Self {
        self.key_ring = Some(key_ring.into());
        self
    }

    /// Set the access token. A token here comes before `GCP_ACCESS_TOKEN`. The
    /// value is a secret and never printed.
    #[must_use]
    pub fn with_token(mut self, token: SecretRef) -> Self {
        self.token = Some(token);
        self
    }

    /// The endpoint.
    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// The deadline of one call.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// The project, when set.
    #[must_use]
    pub fn project(&self) -> Option<&str> {
        self.project.as_deref()
    }

    /// The location, when set.
    #[must_use]
    pub fn location(&self) -> Option<&str> {
        self.location.as_deref()
    }

    /// The key ring, when set.
    #[must_use]
    pub fn key_ring(&self) -> Option<&str> {
        self.key_ring.as_deref()
    }

    /// The configured token, when set.
    #[must_use]
    pub fn token(&self) -> Option<&SecretRef> {
        self.token.as_ref()
    }

    /// The full resource name of the key `key_id` in the configured key ring,
    /// for example
    /// `projects/p/locations/l/keyRings/r/cryptoKeys/k`.
    ///
    /// # Errors
    /// [`KeyStoreError::Config`] when a name is missing.
    pub fn crypto_key_name(&self, key_id: &str) -> Result<String, KeyStoreError> {
        let key_ring = self.require_key_ring()?;
        Ok(format!(
            "{}/keyRings/{key_ring}/cryptoKeys/{key_id}",
            self.parent()?
        ))
    }

    /// The `projects/{project}/locations/{location}` parent.
    ///
    /// # Errors
    /// [`KeyStoreError::Config`] when the project or the location is missing.
    fn parent(&self) -> Result<String, KeyStoreError> {
        let project = require("project", self.project())?;
        let location = require("location", self.location())?;
        Ok(format!("projects/{project}/locations/{location}"))
    }

    /// The key ring name.
    ///
    /// # Errors
    /// [`KeyStoreError::Config`] when the key ring is missing.
    fn require_key_ring(&self) -> Result<&str, KeyStoreError> {
        require("key_ring", self.key_ring())
    }

    /// The `host:port` of the endpoint.
    ///
    /// # Errors
    /// [`KeyStoreError::Config`] for an empty endpoint, an `https` scheme
    /// (the transport does no TLS), a path, or an address that is not an IP
    /// literal with a port.
    fn authority(&self) -> Result<&str, KeyStoreError> {
        let endpoint = self.endpoint.trim();
        let authority = endpoint.strip_prefix("http://").unwrap_or(endpoint);
        if authority.is_empty() {
            return Err(KeyStoreError::Config(
                "the Cloud KMS gRPC endpoint is empty".to_string(),
            ));
        }
        if authority.starts_with("https://") {
            return Err(KeyStoreError::Config(
                "the Cloud KMS gRPC transport is cleartext h2c; an https endpoint is not supported"
                    .to_string(),
            ));
        }
        if authority.contains('/') {
            return Err(KeyStoreError::Config(format!(
                "the Cloud KMS gRPC endpoint {endpoint:?} must not carry a path"
            )));
        }
        if authority.parse::<std::net::SocketAddr>().is_err() {
            return Err(KeyStoreError::Config(format!(
                "the Cloud KMS gRPC endpoint {endpoint:?} needs an IP address and a port"
            )));
        }
        Ok(authority)
    }

    /// Check every required value. The backend calls this once, at
    /// construction, so a call never fails on a missing name.
    ///
    /// # Errors
    /// [`KeyStoreError::Config`] for a missing name or an unusable endpoint.
    fn validate(&self) -> Result<(), KeyStoreError> {
        self.parent()?;
        self.require_key_ring()?;
        self.authority()?;
        Ok(())
    }
}

/// One required value.
///
/// # Errors
/// [`KeyStoreError::Config`] when the value is absent or blank.
fn require<'a>(name: &str, value: Option<&'a str>) -> Result<&'a str, KeyStoreError> {
    match value {
        Some(value) if !value.trim().is_empty() => Ok(value),
        _ => Err(KeyStoreError::Config(format!(
            "the Cloud KMS gRPC backend needs {name}"
        ))),
    }
}

/// Google Cloud KMS over gRPC.
///
/// `key_name` is the full resource name of the encryption key, for example
/// `projects/p/locations/l/keyRings/r/cryptoKeys/k`. The `Encrypt` and
/// `Decrypt` calls of the envelope sealer use it.
#[derive(Debug)]
pub struct GcpKmsGrpcBackend {
    key_name: String,
    config: GcpKmsGrpcConfig,
}

impl GcpKmsGrpcBackend {
    /// A backend for the encryption key `key_name` with the given settings.
    ///
    /// # Errors
    /// [`KeyStoreError::Config`] when the settings are not complete; see
    /// [`GcpKmsGrpcConfig`].
    pub fn new(
        key_name: impl Into<String>,
        config: GcpKmsGrpcConfig,
    ) -> Result<Self, KeyStoreError> {
        config.validate()?;
        let key_name = key_name.into();
        if key_name.trim().is_empty() {
            return Err(KeyStoreError::Config(
                "the Cloud KMS gRPC backend needs the key name".to_string(),
            ));
        }
        Ok(Self { key_name, config })
    }

    /// The settings of this backend.
    #[must_use]
    pub fn config(&self) -> &GcpKmsGrpcConfig {
        &self.config
    }

    /// The access token: the configured token first, then `GCP_ACCESS_TOKEN`.
    ///
    /// The metadata server is not read here. The h2c transport reaches only a
    /// cleartext endpoint, which is a local emulator, and the emulator accepts
    /// any token. A token is a secret and never printed.
    ///
    /// # Errors
    /// [`KeyStoreError::KekUnavailable`] when no token is configured and the
    /// environment variable is absent or blank.
    fn access_token(&self) -> Result<String, KeyStoreError> {
        if let Some(token) = self.config.token() {
            let bytes = token.resolve()?;
            let text = std::str::from_utf8(&bytes).map_err(|_| {
                KeyStoreError::Config("the Cloud KMS access token is not UTF-8".to_string())
            })?;
            if !text.trim().is_empty() {
                return Ok(text.to_string());
            }
        }
        if let Ok(token) = std::env::var("GCP_ACCESS_TOKEN") {
            if !token.trim().is_empty() {
                return Ok(token);
            }
        }
        Err(KeyStoreError::KekUnavailable(
            "no Cloud KMS access token: set GCP_ACCESS_TOKEN or use with_token".to_string(),
        ))
    }

    /// Send one unary request on the Cloud KMS gRPC service.
    ///
    /// The transport reads a success answer under the client size limit and a
    /// failed answer only to its error-body cap, so this never loads a whole
    /// error body.
    ///
    /// # Errors
    /// [`KeyStoreError::Config`] when the endpoint or the token is not
    /// usable. [`KeyStoreError::Storage`] for a transport failure or a non-OK
    /// `grpc-status`.
    async fn unary<Req, Res>(&self, method: &str, request: Req) -> Result<Res, KeyStoreError>
    where
        Req: buffa::Message + Send + 'static,
        Res: buffa::Message + Send + 'static,
    {
        let authority = self.config.authority()?;
        let token = self.access_token()?;
        let timeout = self.config.timeout();
        let transport: Arc<dyn Transport> = Arc::new(
            H2Transport::new()
                .with_connect_timeout(timeout)
                .with_handshake_timeout(timeout)
                .with_read_timeout(timeout),
        );
        let client = Client::<Req, Res>::new(
            transport,
            &format!("http://{authority}/{SERVICE}/{method}"),
            ProcedureCodecs::only(ProtoCodec),
            ClientOptions::new()
                .with_grpc()
                .with_header("authorization".to_string(), format!("Bearer {token}"))
                .with_timeout(timeout),
        )
        .map_err(|e| KeyStoreError::Config(format!("the Cloud KMS gRPC client: {e}")))?;
        let response = client
            .unary(RpcCtx::background(), Request::new(request))
            .await
            .map_err(|e| {
                // The trace's context is the `ConnectError` of the failed call,
                // so the gRPC code and its message are both at hand.
                let cause = e.current_context();
                KeyStoreError::Storage(format!(
                    "Cloud KMS gRPC {SERVICE}/{method}: {}: {}",
                    cause.code(),
                    cause.message()
                ))
            })?;
        Ok(response.msg)
    }

    /// Call a Cloud KMS method with raw messages.
    ///
    /// This is the plain call. The typed methods below use it, and a test uses
    /// it to record the answer of a method that Cloud KMS v1 does not define,
    /// such as `GenerateDataKey`.
    ///
    /// # Errors
    /// The same errors as [`Self::unary`].
    pub async fn invoke<Req, Res>(&self, method: &str, request: Req) -> Result<Res, KeyStoreError>
    where
        Req: buffa::Message + Send + 'static,
        Res: buffa::Message + Send + 'static,
    {
        self.unary(method, request).await
    }

    /// Create the key ring `key_ring_id` in the configured project and
    /// location.
    ///
    /// # Errors
    /// [`KeyStoreError::Storage`] for a non-OK answer, for example
    /// `already_exists`.
    pub async fn create_key_ring(&self, key_ring_id: &str) -> Result<KeyRing, KeyStoreError> {
        let request = proto::google::cloud::kms::v1::CreateKeyRingRequest {
            parent: self.config.parent()?,
            key_ring_id: key_ring_id.to_string(),
            key_ring: buffa::MessageField::some(KeyRing::default()),
            ..Default::default()
        };
        self.unary("CreateKeyRing", request).await
    }

    /// Create the symmetric encryption key `key_id` in the configured key
    /// ring, with one enabled version.
    ///
    /// # Errors
    /// [`KeyStoreError::Config`] when the key ring is absent.
    /// [`KeyStoreError::Storage`] for a non-OK answer.
    pub async fn create_key(&self, key_id: &str) -> Result<CryptoKey, KeyStoreError> {
        let key_ring = self.config.require_key_ring()?;
        let template = proto::google::cloud::kms::v1::CryptoKeyVersionTemplate {
            algorithm: buffa::EnumValue::Known(
                crypto_key_version::CryptoKeyVersionAlgorithm::GOOGLE_SYMMETRIC_ENCRYPTION,
            ),
            ..Default::default()
        };
        let key = CryptoKey {
            purpose: buffa::EnumValue::Known(CryptoKeyPurpose::ENCRYPT_DECRYPT),
            version_template: buffa::MessageField::some(template),
            ..Default::default()
        };
        let request = proto::google::cloud::kms::v1::CreateCryptoKeyRequest {
            parent: format!("{}/keyRings/{key_ring}", self.config.parent()?),
            crypto_key_id: key_id.to_string(),
            crypto_key: buffa::MessageField::some(key),
            ..Default::default()
        };
        self.unary("CreateCryptoKey", request).await
    }

    /// Describe the key at `key_name`.
    ///
    /// # Errors
    /// [`KeyStoreError::Storage`] for a non-OK answer, for example
    /// `not_found`.
    pub async fn describe_key(&self, key_name: &str) -> Result<CryptoKey, KeyStoreError> {
        self.unary(
            "GetCryptoKey",
            GetCryptoKeyRequest {
                name: key_name.to_string(),
                ..Default::default()
            },
        )
        .await
    }

    /// Describe the key version at `version_name`.
    ///
    /// # Errors
    /// [`KeyStoreError::Storage`] for a non-OK answer.
    pub async fn describe_key_version(
        &self,
        version_name: &str,
    ) -> Result<CryptoKeyVersion, KeyStoreError> {
        self.unary(
            "GetCryptoKeyVersion",
            GetCryptoKeyVersionRequest {
                name: version_name.to_string(),
                ..Default::default()
            },
        )
        .await
    }

    /// Destroy the key version at `version_name`. Cloud KMS destroys the key
    /// material, and the version keeps the `DESTROYED` state.
    ///
    /// # Errors
    /// [`KeyStoreError::Storage`] for a non-OK answer.
    pub async fn destroy_key_version(
        &self,
        version_name: &str,
    ) -> Result<CryptoKeyVersion, KeyStoreError> {
        self.unary(
            "DestroyCryptoKeyVersion",
            DestroyCryptoKeyVersionRequest {
                name: version_name.to_string(),
                ..Default::default()
            },
        )
        .await
    }

    /// Ask Cloud KMS to delete the key at `key_name`. The call answers with a
    /// long-running operation; a failed operation becomes a storage error.
    ///
    /// # Errors
    /// [`KeyStoreError::Storage`] for a non-OK answer or a failed operation.
    pub async fn schedule_key_deletion(&self, key_name: &str) -> Result<(), KeyStoreError> {
        let request = proto::google::cloud::kms::v1::DeleteCryptoKeyRequest {
            name: key_name.to_string(),
            ..Default::default()
        };
        let operation: proto::google::longrunning::Operation =
            self.unary("DeleteCryptoKey", request).await?;
        if let Some(proto::google::longrunning::operation::Result::Error(error)) = operation.result
        {
            return Err(KeyStoreError::Storage(format!(
                "Cloud KMS DeleteCryptoKey {}: {}",
                error.code, error.message
            )));
        }
        Ok(())
    }

    /// The `Encrypt` call of the trait.
    ///
    /// # Errors
    /// The errors of [`Self::unary`].
    async fn encrypt_dek(
        &self,
        plaintext_dek: &[u8],
        context: &[u8],
    ) -> Result<Vec<u8>, KeyStoreError> {
        let request = EncryptRequest {
            name: self.key_name.clone(),
            plaintext: plaintext_dek.to_vec(),
            additional_authenticated_data: context.to_vec(),
            ..Default::default()
        };
        let response: EncryptResponse = self.unary("Encrypt", request).await?;
        Ok(response.ciphertext)
    }

    /// The `Decrypt` call.
    ///
    /// # Errors
    /// The errors of [`Self::unary`].
    async fn decrypt_dek(
        &self,
        wrapped_dek: &[u8],
        context: &[u8],
    ) -> Result<SecureBytes, KeyStoreError> {
        let request = DecryptRequest {
            name: self.key_name.clone(),
            ciphertext: wrapped_dek.to_vec(),
            additional_authenticated_data: context.to_vec(),
            ..Default::default()
        };
        let response: DecryptResponse = self.unary("Decrypt", request).await?;
        Ok(SecureBytes::new(response.plaintext))
    }
}

impl KmsBackend for GcpKmsGrpcBackend {
    fn key_id(&self) -> &str {
        &self.key_name
    }

    fn encrypt<'a>(
        &'a self,
        plaintext_dek: &'a [u8],
        context: &'a [u8],
    ) -> SealFuture<'a, Vec<u8>> {
        Box::pin(async move { self.encrypt_dek(plaintext_dek, context).await })
    }

    fn decrypt<'a>(
        &'a self,
        wrapped_dek: &'a [u8],
        context: &'a [u8],
    ) -> SealFuture<'a, SecureBytes> {
        Box::pin(async move { self.decrypt_dek(wrapped_dek, context).await })
    }
}
