//! JWT management module for token lifecycle handling.

use chrono::{DateTime, Utc};
use jwt_simple::prelude::{
    EdDSAKeyPairLike, EdDSAPublicKeyLike,
    RSAKeyPairLike, RSAPublicKeyLike,
    ECDSAP256KeyPairLike, ECDSAP256PublicKeyLike,
};
use serde::{Deserialize, Serialize};

use crate::ConfidentialText;

// ===========================================================================
// Feature 01: JWT Verifier — cryptographic signature verification
// ===========================================================================

/// Supported JWT signature algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JwtAlgorithm {
    /// EdDSA (Ed25519)
    EdDSA,
    /// RS256 (RSA with SHA-256)
    RS256,
    /// ES256 (ECDSA P-256 with SHA-256)
    ES256,
}

/// Source of a public key for verification.
#[derive(Debug, Clone)]
pub enum PublicKeySource {
    /// PEM-encoded public key.
    Pem(String),
}

/// Configuration for JWT verification.
#[derive(Debug, Clone)]
pub struct JwtVerifierConfig {
    /// Allowed signature algorithms.
    pub allowed_algorithms: Vec<JwtAlgorithm>,
    /// Expected issuer claim (validated if set).
    pub issuer: Option<String>,
    /// Expected audience claim (validated if set).
    pub audience: Option<String>,
    /// Key ID override for JWKS-based verification.
    pub key_id: Option<String>,
    /// Public key material.
    pub public_key: PublicKeySource,
}

/// Verified public key — one variant per supported algorithm.
#[derive(Clone)]
enum VerifiedPublicKey {
    Ed25519(jwt_simple::prelude::Ed25519PublicKey),
    RS256(jwt_simple::prelude::RS256PublicKey),
    ES256(jwt_simple::prelude::ES256PublicKey),
}

impl VerifiedPublicKey {
    fn from_pem(pem: &str) -> Result<Self, JwtError> {
        // Try each key type until one succeeds
        if let Ok(pk) = jwt_simple::prelude::Ed25519PublicKey::from_pem(pem) {
            return Ok(Self::Ed25519(pk));
        }
        if let Ok(pk) = jwt_simple::prelude::RS256PublicKey::from_pem(pem) {
            return Ok(Self::RS256(pk));
        }
        if let Ok(pk) = jwt_simple::prelude::ES256PublicKey::from_pem(pem) {
            return Ok(Self::ES256(pk));
        }
        Err(JwtError::InvalidPublicKey(
            "could not parse PEM as Ed25519, RSA, or EC public key".into(),
        ))
    }

    fn algorithm(&self) -> JwtAlgorithm {
        match self {
            Self::Ed25519(_) => JwtAlgorithm::EdDSA,
            Self::RS256(_) => JwtAlgorithm::RS256,
            Self::ES256(_) => JwtAlgorithm::ES256,
        }
    }

    fn alg_name(&self) -> &'static str {
        match self {
            Self::Ed25519(_) => "EdDSA",
            Self::RS256(_) => "RS256",
            Self::ES256(_) => "ES256",
        }
    }

    /// Verify a token using the correct algorithm.
    fn verify_token(
        &self,
        token: &str,
        options: Option<jwt_simple::prelude::VerificationOptions>,
    ) -> Result<jwt_simple::prelude::JWTClaims<serde_json::Value>, JwtError> {
        match self {
            Self::Ed25519(pk) => pk
                .verify_token(token, options)
                .map_err(|e| JwtError::SignatureVerificationFailed(e.to_string())),
            Self::RS256(pk) => pk
                .verify_token(token, options)
                .map_err(|e| JwtError::SignatureVerificationFailed(e.to_string())),
            Self::ES256(pk) => pk
                .verify_token(token, options)
                .map_err(|e| JwtError::SignatureVerificationFailed(e.to_string())),
        }
    }
}

/// Claims extracted after successful JWT verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedClaims {
    /// Subject identifier (required).
    pub sub: String,
    /// Issuer.
    pub iss: String,
    /// Audience (first if multiple).
    pub aud: String,
    /// Expiration time.
    pub exp: DateTime<Utc>,
    /// Issued-at time.
    pub iat: DateTime<Utc>,
    /// Not-before time (if present).
    pub nbf: Option<DateTime<Utc>>,
    /// JWT ID (if present).
    pub jti: Option<String>,
    /// Scope string (from custom claims).
    pub scope: Option<String>,
    /// OIDC nonce (if present).
    pub nonce: Option<String>,
    /// Custom claims (standard claims stripped).
    #[serde(flatten)]
    pub custom: serde_json::Map<String, serde_json::Value>,
}

impl VerifiedClaims {
    /// Check if the nonce in the claims matches the expected nonce (Feature 07).
    #[must_use]
    pub fn verify_nonce(&self, expected_nonce: &str) -> bool {
        self.nonce
            .as_ref()
            .is_some_and(|n| n == expected_nonce)
    }
}

/// JWT verifier — validates signatures and claims using jwt-simple.
pub struct JwtVerifier {
    config: JwtVerifierConfig,
    public_key: VerifiedPublicKey,
}

impl JwtVerifier {
    /// Create a verifier from configuration.
    ///
    /// # Errors
    ///
    /// Returns `JwtError::InvalidPublicKey` if the key material cannot be parsed.
    pub fn from_config(config: JwtVerifierConfig) -> Result<Self, JwtError> {
        let public_key = match &config.public_key {
            PublicKeySource::Pem(pem) => VerifiedPublicKey::from_pem(pem)?,
        };
        Ok(Self { config, public_key })
    }

    /// Build VerificationOptions from the config.
    fn build_options(&self) -> jwt_simple::prelude::VerificationOptions {
        let mut opts = jwt_simple::prelude::VerificationOptions::default();

        if let Some(ref issuer) = self.config.issuer {
            let mut issuers = std::collections::HashSet::new();
            issuers.insert(issuer.clone());
            opts.allowed_issuers = Some(issuers);
        }
        if let Some(ref audience) = self.config.audience {
            let mut audiences = std::collections::HashSet::new();
            audiences.insert(audience.clone());
            opts.allowed_audiences = Some(audiences);
        }
        if let Some(ref key_id) = self.config.key_id {
            opts.required_key_id = Some(key_id.clone());
        }

        opts
    }

    /// Verify a JWT token, returning parsed claims on success.
    ///
    /// # Errors
    ///
    /// Returns `JwtError` if the signature is invalid, an algorithm is not allowed,
    /// or claim validation fails.
    pub fn verify(&self, token: &str) -> Result<VerifiedClaims, JwtError> {
        // Parse header to check algorithm before spending crypto cycles
        let header_alg = decode_header_algorithm(token)?;
        let expected_alg = self.public_key.alg_name();
        if header_alg != expected_alg {
            return Err(JwtError::AlgorithmNotAllowed(format!(
                "token uses {header_alg}, expected {expected_alg}"
            )));
        }

        // Check the algorithm is in the allowed list
        if !self
            .config
            .allowed_algorithms
            .contains(&self.public_key.algorithm())
        {
            return Err(JwtError::AlgorithmNotAllowed(format!(
                "{expected_alg} is not in the allowed algorithms list"
            )));
        }

        let options = self.build_options();
        let claims = self.public_key.verify_token(token, Some(options))?;

        to_verified_claims(&claims)
    }

    /// Verify with an explicit key ID override (used with JWKS rotation).
    ///
    /// # Errors
    ///
    /// Returns `JwtError` if verification fails.
    pub fn verify_with_key(&self, token: &str, _kid: &str) -> Result<VerifiedClaims, JwtError> {
        // When using a specific key from JWKS, the caller has already selected
        // the right public key. The verification logic is identical.
        self.verify(token)
    }

    /// Get the configured key ID.
    #[must_use]
    pub fn key_id(&self) -> Option<&str> {
        self.config.key_id.as_deref()
    }
}

/// JWT signing key pair for the server module.
pub enum JwtSigningKey {
    /// Ed25519 key pair (default, recommended).
    Ed25519(jwt_simple::prelude::Ed25519KeyPair),
    /// RSA key pair (RS256).
    RS256(jwt_simple::prelude::RS256KeyPair),
    /// EC P-256 key pair (ES256).
    ES256(jwt_simple::prelude::ES256KeyPair),
}

impl JwtSigningKey {
    /// Generate a new Ed25519 key pair.
    #[must_use]
    pub fn generate_ed25519() -> Self {
        Self::Ed25519(jwt_simple::prelude::Ed25519KeyPair::generate())
    }

    /// Sign a set of claims, returning a JWT string.
    ///
    /// # Errors
    ///
    /// Returns `JwtError::GenerationError` if signing fails.
    pub fn sign_claims(&self, claims: &serde_json::Value) -> Result<String, JwtError> {
        use jwt_simple::prelude::{Claims, Duration};

        // Token lifetime: derive from an absolute `exp` (epoch seconds) when the
        // caller supplies one, otherwise default to 1 hour.
        let duration = claims
            .get("exp")
            .and_then(serde_json::Value::as_u64)
            .map(|exp| {
                let now = jwt_simple::prelude::Clock::now_since_epoch().as_secs();
                Duration::from_secs(exp.saturating_sub(now).max(1))
            })
            .unwrap_or_else(|| Duration::from_secs(3600));

        // Application (custom) claims = everything except the registered claims
        // jwt-simple manages itself — otherwise those keys would appear twice in
        // the payload. Custom claims are what downstream `VerifiedClaims.custom`
        // reads back (e.g. Bitwarden's `sstamp`, `device`, `email`, `premium`).
        let mut custom = claims.clone();
        if let Some(obj) = custom.as_object_mut() {
            for key in ["sub", "iss", "aud", "exp", "iat", "nbf", "jti"] {
                obj.remove(key);
            }
        }

        let mut builder = Claims::with_custom_claims(custom, duration);

        // Apply standard claims from the JSON value
        if let Some(sub) = claims.get("sub").and_then(|v| v.as_str()) {
            builder = builder.with_subject(sub);
        }
        if let Some(iss) = claims.get("iss").and_then(|v| v.as_str()) {
            builder = builder.with_issuer(iss);
        }
        if let Some(aud) = claims.get("aud").and_then(|v| v.as_str()) {
            builder = builder.with_audience(aud);
        }

        match self {
            Self::Ed25519(kp) => kp
                .sign(builder)
                .map_err(|e| JwtError::GenerationError(e.to_string())),
            Self::RS256(kp) => kp
                .sign(builder)
                .map_err(|e| JwtError::GenerationError(e.to_string())),
            Self::ES256(kp) => kp
                .sign(builder)
                .map_err(|e| JwtError::GenerationError(e.to_string())),
        }
    }

    /// Get the public key as PEM.
    ///
    /// # Errors
    ///
    /// Returns `JwtError::GenerationError` if PEM encoding fails.
    pub fn public_key_pem(&self) -> Result<String, JwtError> {
        match self {
            Self::Ed25519(kp) => Ok(kp.public_key().to_pem()),
            Self::RS256(kp) => kp.public_key().to_pem()
                .map_err(|e| JwtError::GenerationError(e.to_string())),
            Self::ES256(kp) => kp.public_key().to_pem()
                .map_err(|e| JwtError::GenerationError(e.to_string())),
        }
    }
}

// ===========================================================================
// Internal helpers
// ===========================================================================

/// Decode the JWT header to extract the algorithm string.
fn decode_header_algorithm(token: &str) -> Result<String, JwtError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(JwtError::InvalidTokenFormat);
    }
    let payload = base64_decode(parts[0])?;
    let header: serde_json::Value =
        serde_json::from_slice(&payload).map_err(|e| JwtError::Json(e))?;
    header
        .get("alg")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| JwtError::AlgorithmNotAllowed("missing alg in header".into()))
}

/// Convert jwt-simple JWTClaims to our VerifiedClaims.
fn to_verified_claims(
    claims: &jwt_simple::prelude::JWTClaims<serde_json::Value>,
) -> Result<VerifiedClaims, JwtError> {
    let sub = claims
        .subject
        .clone()
        .ok_or(JwtError::InvalidSubject)?;

    // Build custom claims map, stripping standard claims
    let raw: serde_json::Value =
        serde_json::to_value(claims).map_err(|e| JwtError::Json(e))?;

    // Extract scope before consuming raw
    let scope = raw
        .get("scope")
        .and_then(|v| v.as_str())
        .map(String::from);

    let custom = if let serde_json::Value::Object(map) = raw {
        let standard_keys = [
            "sub", "iss", "aud", "exp", "iat", "nbf", "jti", "nonce",
        ];
        map.into_iter()
            .filter(|(k, _)| !standard_keys.contains(&k.as_str()))
            .collect()
    } else {
        serde_json::Map::new()
    };

    let unix_ts_to_datetime = |ts: Option<jwt_simple::prelude::UnixTimeStamp>| {
        ts.map(|t| DateTime::from_timestamp(t.as_secs() as i64, 0))
            .flatten()
    };

    Ok(VerifiedClaims {
        sub,
        iss: claims.issuer.clone().unwrap_or_default(),
        aud: match claims.audiences.as_ref() {
            Some(jwt_simple::prelude::Audiences::AsString(s)) => s.clone(),
            Some(jwt_simple::prelude::Audiences::AsSet(set)) => {
                set.iter().next().cloned().unwrap_or_default()
            }
            None => String::new(),
        },
        exp: unix_ts_to_datetime(claims.expires_at).unwrap_or_else(Utc::now),
        iat: unix_ts_to_datetime(claims.issued_at).unwrap_or_else(Utc::now),
        nbf: unix_ts_to_datetime(claims.invalid_before),
        jti: claims.jwt_id.clone(),
        scope,
        nonce: claims.nonce.clone(),
        custom,
    })
}

// ===========================================================================
// Original types (preserved for backward compatibility)
// ===========================================================================

/// A JWT token with metadata for lifecycle management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtToken {
    /// The raw JWT token string.
    pub token: ConfidentialText,
    /// Refresh token (if available).
    pub refresh_token: Option<ConfidentialText>,
    /// Token expiration timestamp (Unix epoch seconds).
    pub expires_at: i64,
    /// Token scope.
    pub scope: Option<String>,
    /// Token audience.
    pub audience: Option<String>,
    /// Token issuer.
    pub issuer: Option<String>,
    /// When the token was created.
    pub created_at: i64,
}

impl JwtToken {
    /// Create a new JWT token from parts.
    #[must_use]
    pub fn from_parts(
        token: String,
        refresh_token: Option<String>,
        expires_at: i64,
        scope: Option<String>,
        audience: Option<String>,
        issuer: Option<String>,
    ) -> Self {
        let now = Utc::now().timestamp();
        Self {
            token: ConfidentialText::new(token),
            refresh_token: refresh_token.map(ConfidentialText::new),
            expires_at,
            scope,
            audience,
            issuer,
            created_at: now,
        }
    }

    /// Create from a raw token string by parsing the JWT payload.
    ///
    /// # Errors
    ///
    /// Returns a `JwtError` if the token cannot be parsed or is missing required claims.
    pub fn from_token(token: String) -> Result<Self, JwtError> {
        // Parse the JWT payload to extract claims
        let claims = decode_claims(&token)?;

        let expires_at = claims.exp.ok_or(JwtError::MissingExpiration)?;

        Ok(Self {
            token: ConfidentialText::new(token),
            refresh_token: None, // Refresh token not embedded in JWT
            expires_at,
            scope: claims
                .custom
                .get("scope")
                .and_then(|v| v.as_str().map(String::from)),
            audience: claims
                .custom
                .get("aud")
                .and_then(|v| v.as_str().map(String::from)),
            issuer: claims
                .custom
                .get("iss")
                .and_then(|v| v.as_str().map(String::from)),
            created_at: Utc::now().timestamp(),
        })
    }

    /// Create with a refresh token.
    #[must_use]
    pub fn with_refresh_token(mut self, refresh_token: String) -> Self {
        self.refresh_token = Some(ConfidentialText::new(refresh_token));
        self
    }

    /// Check if the token is expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        let now = Utc::now().timestamp();
        now >= self.expires_at
    }

    /// Check if the token expires within the given buffer (in seconds).
    #[must_use]
    pub fn expires_within(&self, buffer_seconds: i64) -> bool {
        let now = Utc::now().timestamp();
        now + buffer_seconds >= self.expires_at
    }

    /// Get seconds until expiration.
    #[must_use]
    pub fn expires_in(&self) -> i64 {
        let now = Utc::now().timestamp();
        (self.expires_at - now).max(0)
    }

    /// Get the refresh token if available.
    #[must_use]
    pub fn refresh_token(&self) -> Option<String> {
        self.refresh_token.as_ref().map(ConfidentialText::get)
    }

    /// Get the access token.
    #[must_use]
    pub fn access_token(&self) -> String {
        self.token.get()
    }
}

/// JWT Manager for handling token lifecycle.
pub struct JwtManager {
    /// Current token.
    token: Option<JwtToken>,
    /// Refresh buffer in seconds (default 5 minutes).
    refresh_buffer: i64,
    /// Token storage key.
    storage_key: String,
}

impl Default for JwtManager {
    fn default() -> Self {
        Self::new()
    }
}

impl JwtManager {
    /// Create a new JWT manager with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            token: None,
            refresh_buffer: 300, // 5 minutes
            storage_key: String::from("jwt:token"),
        }
    }

    /// Create with a custom refresh buffer.
    #[must_use]
    pub fn with_refresh_buffer(mut self, buffer_seconds: i64) -> Self {
        self.refresh_buffer = buffer_seconds;
        self
    }

    /// Create with a custom storage key.
    #[must_use]
    pub fn with_storage_key(mut self, key: impl Into<String>) -> Self {
        self.storage_key = key.into();
        self
    }

    /// Set the JWT token.
    pub fn set_token(&mut self, token: JwtToken) {
        self.token = Some(token);
    }

    /// Get the current token without checking expiration.
    #[must_use]
    pub fn get_token(&self) -> Option<&JwtToken> {
        self.token.as_ref()
    }

    /// Clear the stored token.
    pub fn clear_token(&mut self) {
        self.token = None;
    }

    /// Get a valid token, refreshing if necessary.
    ///
    /// Returns the access token string if available and valid.
    ///
    /// # Errors
    ///
    /// Returns a `JwtError` if there is no token, no refresh token, or if the refresh function fails.
    pub fn get_valid_token<F>(&mut self, refresh_fn: F) -> Result<String, JwtError>
    where
        F: FnOnce(String) -> Result<JwtToken, JwtError>,
    {
        // Check if we have a token
        let Some(token) = &self.token else {
            return Err(JwtError::NoToken);
        };

        // Check if token needs refresh (expired or within buffer)
        if token.is_expired() || token.expires_within(self.refresh_buffer) {
            // Need to refresh
            let Some(refresh_token) = token.refresh_token() else {
                return Err(JwtError::NoRefreshToken);
            };

            // Call the refresh function
            let new_token = refresh_fn(refresh_token)?;
            self.token = Some(new_token);
        }

        // Return the current access token
        self.token
            .as_ref()
            .map(JwtToken::access_token)
            .ok_or(JwtError::NoToken)
    }

    /// Refresh the token if needed (within buffer or expired).
    ///
    /// # Errors
    ///
    /// Returns a `JwtError` if there is no refresh token or if the refresh function fails.
    pub fn refresh_if_needed<F>(&mut self, refresh_fn: F) -> Result<bool, JwtError>
    where
        F: FnOnce(String) -> Result<JwtToken, JwtError>,
    {
        let Some(token) = &self.token else {
            return Ok(false);
        };

        if !token.is_expired() && !token.expires_within(self.refresh_buffer) {
            return Ok(false); // No refresh needed
        }

        let Some(refresh_token) = token.refresh_token() else {
            return Err(JwtError::NoRefreshToken);
        };

        let new_token = refresh_fn(refresh_token)?;
        self.token = Some(new_token);
        Ok(true)
    }

    /// Check if the manager has a valid (non-expired) token.
    #[must_use]
    pub fn has_valid_token(&self) -> bool {
        self.token
            .as_ref()
            .is_some_and(|t| !t.is_expired() && !t.expires_within(self.refresh_buffer))
    }

    /// Get the storage key for persistence.
    #[must_use]
    pub fn storage_key(&self) -> &str {
        &self.storage_key
    }
}

/// Parse claims from a JWT token without full validation.
fn decode_claims(token: &str) -> Result<Claims, JwtError> {
    // Split the token into parts
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(JwtError::InvalidTokenFormat);
    }

    // Decode the payload (second part)
    let payload = base64_decode(parts[1])?;
    let claims: Claims = serde_json::from_slice(&payload)?;

    Ok(claims)
}

/// Base64 decode with URL-safe alphabet.
fn base64_decode(input: &str) -> Result<Vec<u8>, JwtError> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    URL_SAFE_NO_PAD
        .decode(input)
        .map_err(|_| JwtError::DecodeError)
}

/// JWT claims structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Issuer.
    #[serde(rename = "iss", skip_serializing_if = "Option::is_none")]
    pub iss: Option<String>,
    /// Subject.
    #[serde(rename = "sub", skip_serializing_if = "Option::is_none")]
    pub sub: Option<String>,
    /// Audience.
    #[serde(rename = "aud", skip_serializing_if = "Option::is_none")]
    pub aud: Option<String>,
    /// Expiration time (Unix timestamp).
    #[serde(rename = "exp", skip_serializing_if = "Option::is_none")]
    pub exp: Option<i64>,
    /// Not before (Unix timestamp).
    #[serde(rename = "nbf", skip_serializing_if = "Option::is_none")]
    pub nbf: Option<i64>,
    /// Issued at (Unix timestamp).
    #[serde(rename = "iat", skip_serializing_if = "Option::is_none")]
    pub iat: Option<i64>,
    /// JWT ID.
    #[serde(rename = "jti", skip_serializing_if = "Option::is_none")]
    pub jti: Option<String>,
    /// Custom claims.
    #[serde(flatten)]
    pub custom: serde_json::Map<String, serde_json::Value>,
}

impl Claims {
    /// Get the expiration time as [`DateTime`].
    #[must_use]
    pub fn expires_at(&self) -> Option<DateTime<Utc>> {
        self.exp.and_then(|ts| DateTime::from_timestamp(ts, 0))
    }
}

/// JWT-related errors.
#[derive(derive_more::From, Debug)]
pub enum JwtError {
    /// No token available.
    NoToken,
    /// No refresh token available.
    NoRefreshToken,
    /// Token refresh failed.
    #[from(ignore)]
    RefreshFailed(String),
    /// Invalid token format.
    InvalidTokenFormat,
    /// Token is expired.
    TokenExpired,
    /// Missing expiration claim.
    MissingExpiration,
    /// Base64 decode error.
    #[from(ignore)]
    DecodeError,
    /// JSON parse error.
    Json(serde_json::Error),
    /// Token generation error.
    #[from(ignore)]
    GenerationError(String),

    // Feature 01: Verification errors
    /// Signature does not match.
    #[from(ignore)]
    SignatureVerificationFailed(String),
    /// Token uses an algorithm not in allowlist.
    #[from(ignore)]
    AlgorithmNotAllowed(String),
    /// Issuer claim does not match expected.
    #[from(ignore)]
    InvalidIssuer(String),
    /// Audience claim does not match expected.
    #[from(ignore)]
    InvalidAudience(String),
    /// nbf is in the future.
    #[from(ignore)]
    TokenNotYetValid(String),
    /// Public key could not be parsed.
    #[from(ignore)]
    InvalidPublicKey(String),
    /// Subject claim is missing or empty.
    #[from(ignore)]
    InvalidSubject,
}

impl core::fmt::Display for JwtError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            JwtError::NoToken => write!(f, "No JWT token available"),
            JwtError::NoRefreshToken => write!(f, "No refresh token available"),
            JwtError::RefreshFailed(s) => write!(f, "Token refresh failed: {s}"),
            JwtError::InvalidTokenFormat => write!(f, "Invalid JWT token format"),
            JwtError::TokenExpired => write!(f, "JWT token is expired"),
            JwtError::MissingExpiration => write!(f, "JWT token missing expiration claim"),
            JwtError::DecodeError => write!(f, "Base64 decode error"),
            JwtError::Json(e) => write!(f, "JSON parse error: {e}"),
            JwtError::GenerationError(s) => write!(f, "Token generation error: {s}"),
            // Feature 01: verification errors
            JwtError::SignatureVerificationFailed(s) => {
                write!(f, "JWT signature verification failed: {s}")
            }
            JwtError::AlgorithmNotAllowed(s) => {
                write!(f, "JWT algorithm not allowed: {s}")
            }
            JwtError::InvalidIssuer(s) => write!(f, "JWT invalid issuer: {s}"),
            JwtError::InvalidAudience(s) => write!(f, "JWT invalid audience: {s}"),
            JwtError::TokenNotYetValid(s) => write!(f, "JWT not yet valid: {s}"),
            JwtError::InvalidPublicKey(s) => write!(f, "JWT invalid public key: {s}"),
            JwtError::InvalidSubject => write!(f, "JWT missing subject claim"),
        }
    }
}

impl std::error::Error for JwtError {}

