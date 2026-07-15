//! JWKS (JSON Web Key Set) manager.
//!
//! Fetches, caches, and manages JWKS from OIDC providers. Supports key rotation
//! (multiple keys in a single JWKS), lazy TTL-based caching, and conversion to
//! `JwtVerifier` for signature verification.
//!
//! ## Caching Strategy
//! - Cache TTL defaults to 1 hour
//! - `get()` returns cached if valid, fetches otherwise (lazy refresh)
//! - `fetch()` always fetches (force refresh)
//! - `find_key()` auto-fetches if cache is empty or expired
//! - No background refresh — lazy on access only

use std::time::{Duration, Instant};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};

use super::jwt::{JwtError, JwtVerifier, JwtVerifierConfig};

/// A single JSON Web Key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Jwk {
    /// Key ID.
    pub key_id: String,
    /// Key type: "OKP", "RSA", "EC".
    pub key_type: String,
    /// Algorithm: "EdDSA", "RS256", "ES256".
    pub algorithm: String,
    /// Usage: "sig".
    pub use_: String,
    /// Curve: "Ed25519", "P-256" (for OKP/EC).
    pub curve: Option<String>,
    /// X coordinate (base64url-decoded bytes).
    pub x: Option<Vec<u8>>,
    /// Y coordinate (EC only, base64url-decoded).
    pub y: Option<Vec<u8>>,
    /// Modulus (RSA only, base64url-decoded).
    pub modulus: Option<Vec<u8>>,
    /// Exponent (RSA only, base64url-decoded).
    pub exponent: Option<Vec<u8>>,
    /// Raw PEM representation (populated after parsing).
    pub pem: Option<String>,
}

impl Jwk {
    /// Convert this JWK to a PEM public key string.
    ///
    /// # Errors
    ///
    /// Returns `JwksError::UnsupportedKeyType` or `UnsupportedAlgorithm` if
    /// the key type or algorithm is not recognized.
    pub fn to_pem(&self) -> Result<String, JwksError> {
        if let Some(ref pem) = self.pem {
            return Ok(pem.clone());
        }
        match (self.key_type.as_str(), self.algorithm.as_str()) {
            ("OKP", "EdDSA") => {
                let x = self.x.as_ref().ok_or(JwksError::MissingKeyData("x".into()))?;
                let pk = jwt_simple::prelude::Ed25519PublicKey::from_bytes(x)
                    .map_err(|e| JwksError::KeyParseError(e.to_string()))?;
                Ok(pk.to_pem())
            }
            ("RSA", "RS256") => {
                let n = self
                    .modulus
                    .as_ref()
                    .ok_or(JwksError::MissingKeyData("n".into()))?;
                let e = self
                    .exponent
                    .as_ref()
                    .ok_or(JwksError::MissingKeyData("e".into()))?;
                // Build RSAPublicKey from (n, e)
                let pk =
                    jwt_simple::prelude::RSAPublicKey::from_components(n, e)
                        .map_err(|e| JwksError::KeyParseError(e.to_string()))?;
                Ok(pk
                    .to_pem()
                    .map_err(|e| JwksError::KeyParseError(e.to_string()))?)
            }
            ("EC", "ES256") => {
                let x = self.x.as_ref().ok_or(JwksError::MissingKeyData("x".into()))?;
                let y = self.y.as_ref().ok_or(JwksError::MissingKeyData("y".into()))?;
                // Build uncompressed EC point: 0x04 || x || y (65 bytes for P-256)
                let mut point = vec![0x04];
                point.extend_from_slice(x);
                point.extend_from_slice(y);
                let pk = jwt_simple::prelude::ES256PublicKey::from_bytes(&point)
                    .map_err(|e| JwksError::KeyParseError(e.to_string()))?;
                Ok(pk
                    .to_pem()
                    .map_err(|e| JwksError::KeyParseError(e.to_string()))?)
            }
            (kty, alg) => {
                if !matches!(kty, "OKP" | "RSA" | "EC") {
                    Err(JwksError::UnsupportedKeyType(kty.into()))
                } else {
                    Err(JwksError::UnsupportedAlgorithm(alg.into()))
                }
            }
        }
    }

    /// Create a `JwtVerifier` configured to use this key.
    ///
    /// # Errors
    ///
    /// Returns `JwksError` if the key cannot be converted or the verifier
    /// cannot be created.
    pub fn to_verifier(
        &self,
        issuer: Option<&str>,
        audience: Option<&str>,
    ) -> Result<JwtVerifier, JwksError> {
        let pem = self.to_pem()?;
        let alg = algorithm_str_to_jwt_algorithm(&self.algorithm)?;
        let config = JwtVerifierConfig {
            allowed_algorithms: vec![alg],
            issuer: issuer.map(String::from),
            audience: audience.map(String::from),
            key_id: Some(self.key_id.clone()),
            public_key: super::jwt::PublicKeySource::Pem(pem),
        };
        Ok(JwtVerifier::from_config(config)?)
    }
}

/// A JSON Web Key Set.
#[derive(Debug, Clone)]
pub struct Jwks {
    /// Keys in the set.
    pub keys: Vec<Jwk>,
    /// Optional issuer from JWKS endpoint metadata.
    pub issuer: Option<String>,
}

impl Jwks {
    /// Find a key by key ID.
    #[must_use]
    pub fn find_by_kid(&self, kid: &str) -> Option<&Jwk> {
        self.keys.iter().find(|k| k.key_id == kid)
    }

    /// Parse from a JSON string.
    ///
    /// # Errors
    ///
    /// Returns `JwksError::JwksParseError` if JSON is invalid or key data
    /// cannot be decoded.
    pub fn from_json(json: &str) -> Result<Self, JwksError> {
        let raw: RawJwks = serde_json::from_str(json)
            .map_err(|e| JwksError::JwksParseError(e.to_string()))?;

        let mut keys = Vec::with_capacity(raw.keys.len());
        for raw_key in raw.keys {
            let jwk = Jwk {
                key_id: raw_key.kid.unwrap_or_default(),
                key_type: raw_key.kty,
                algorithm: raw_key.alg,
                use_: raw_key.use_.unwrap_or_else(|| String::from("sig")),
                curve: raw_key.crv,
                x: raw_key.x.map(|b64| base64url_decode(&b64))
                    .transpose()
                    .map_err(|e| JwksError::JwksParseError(e))?,
                y: raw_key.y.map(|b64| base64url_decode(&b64))
                    .transpose()
                    .map_err(|e| JwksError::JwksParseError(e))?,
                modulus: raw_key.n.map(|b64| base64url_decode(&b64))
                    .transpose()
                    .map_err(|e| JwksError::JwksParseError(e))?,
                exponent: raw_key.e.map(|b64| base64url_decode(&b64))
                    .transpose()
                    .map_err(|e| JwksError::JwksParseError(e))?,
                pem: None,
            };
            keys.push(jwk);
        }

        Ok(Self {
            keys,
            issuer: None,
        })
    }

    /// Serialize to a JSON string.
    ///
    /// # Errors
    ///
    /// Returns `JwksError::JwksParseError` if serialization fails.
    pub fn to_json(&self) -> Result<String, JwksError> {
        let raw = RawJwks {
            keys: self
                .keys
                .iter()
                .map(|k| RawJwk {
                    kty: k.key_type.clone(),
                    kid: if k.key_id.is_empty() {
                        None
                    } else {
                        Some(k.key_id.clone())
                    },
                    alg: k.algorithm.clone(),
                    use_: Some(k.use_.clone()),
                    crv: k.curve.clone(),
                    x: k.x.as_ref().map(|b| URL_SAFE_NO_PAD.encode(b)),
                    y: k.y.as_ref().map(|b| URL_SAFE_NO_PAD.encode(b)),
                    n: k.modulus.as_ref().map(|b| URL_SAFE_NO_PAD.encode(b)),
                    e: k.exponent.as_ref().map(|b| URL_SAFE_NO_PAD.encode(b)),
                })
                .collect(),
        };
        serde_json::to_string(&raw).map_err(|e| JwksError::JwksParseError(e.to_string()))
    }
}

/// Raw JWKS JSON structure for deserialization.
#[derive(Debug, Deserialize)]
#[derive(Serialize)]
struct RawJwks {
    keys: Vec<RawJwk>,
}

/// Raw JWK JSON structure for deserialization.
#[derive(Debug, Serialize, Deserialize)]
struct RawJwk {
    kty: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    kid: Option<String>,
    alg: String,
    #[serde(rename = "use", skip_serializing_if = "Option::is_none")]
    use_: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    crv: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    x: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    y: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    n: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    e: Option<String>,
}

/// JWKS manager — fetches, caches, and provides keys for JWT verification.
pub struct JwksManager {
    jwks_url: String,
    cached_jwks: Option<(Jwks, Instant)>,
    cache_ttl: Duration,
}

impl JwksManager {
    /// Create a new JWKS manager.
    #[must_use]
    pub fn new(jwks_url: String, cache_ttl: Option<Duration>) -> Self {
        Self {
            jwks_url,
            cached_jwks: None,
            cache_ttl: cache_ttl.unwrap_or(Duration::from_secs(3600)),
        }
    }

    /// Fetch JWKS from the URL, replacing cache.
    ///
    /// # Errors
    ///
    /// Returns `JwksError::JwksFetchFailed` if the HTTP request fails, or
    /// `JwksParseError` if the response cannot be parsed.
    pub async fn fetch(&mut self) -> Result<&Jwks, JwksError> {
        let json = fetch_jwks(&self.jwks_url).await?;
        let mut jwks = Jwks::from_json(&json)?;
        // If the URL contains an issuer hint, store it
        if let Some(issuer) = extract_issuer_from_url(&self.jwks_url) {
            jwks.issuer = Some(issuer);
        }
        self.cached_jwks = Some((jwks, Instant::now()));
        Ok(&self.cached_jwks.as_ref().unwrap().0)
    }

    /// Get the cached JWKS, refreshing if expired.
    ///
    /// # Errors
    ///
    /// Returns `JwksError` on fetch or parse failure.
    pub async fn get(&mut self) -> Result<&Jwks, JwksError> {
        if self.is_cache_valid() {
            return Ok(&self.cached_jwks.as_ref().unwrap().0);
        }
        self.fetch().await
    }

    /// Find a key by key ID, fetching if cache is empty or expired.
    ///
    /// # Errors
    ///
    /// Returns `JwksError` on fetch/parse failure, or `KeyNotFound` if the
    /// kid does not exist in the JWKS.
    pub async fn find_key(&mut self, kid: &str) -> Result<&Jwk, JwksError> {
        let jwks = self.get().await?;
        jwks.find_by_kid(kid)
            .ok_or_else(|| JwksError::KeyNotFound(kid.into()))
    }

    /// Get a JWT verifier configured with the current JWKS.
    ///
    /// This uses the first key in the JWKS. For specific key selection,
    /// use `find_key()` then `Jwk::to_verifier()`.
    ///
    /// # Errors
    ///
    /// Returns `JwksError` on fetch/parse failure.
    pub async fn verifier(&mut self) -> Result<JwtVerifier, JwksError> {
        let jwks = self.get().await?;
        let first_key = jwks
            .keys
            .first()
            .ok_or(JwksError::KeyNotFound("no keys in JWKS".into()))?;
        first_key.to_verifier(jwks.issuer.as_deref(), None)
    }

    /// Get a JWT verifier for a specific key ID.
    ///
    /// # Errors
    ///
    /// Returns `JwksError` if the key is not found or cannot be converted.
    pub async fn verifier_for_key(
        &mut self,
        kid: &str,
    ) -> Result<JwtVerifier, JwksError> {
        // Ensure cache is populated first
        let _ = self.get().await?;

        // Extract issuer while we have mutable access
        let issuer = self.cached_jwks
            .as_ref()
            .and_then(|(jwks, _)| jwks.issuer.clone());

        // Now get the key (immutable borrow is fine)
        let jwk = self
            .cached_jwks
            .as_ref()
            .and_then(|(jwks, _)| jwks.find_by_kid(kid))
            .ok_or_else(|| JwksError::KeyNotFound(kid.into()))?;

        let pem = jwk.to_pem()?;
        let alg = algorithm_str_to_jwt_algorithm(&jwk.algorithm)?;

        let config = JwtVerifierConfig {
            allowed_algorithms: vec![alg],
            issuer,
            audience: None,
            key_id: Some(jwk.key_id.clone()),
            public_key: super::jwt::PublicKeySource::Pem(pem),
        };
        JwtVerifier::from_config(config)
            .map_err(JwksError::Jwt)
    }

    /// Clear the cached JWKS.
    pub fn clear_cache(&mut self) {
        self.cached_jwks = None;
    }

    /// Check if the cache is still valid.
    fn is_cache_valid(&self) -> bool {
        self.cached_jwks
            .as_ref()
            .is_some_and(|(_, fetched_at)| fetched_at.elapsed() < self.cache_ttl)
    }
}

/// Convert an algorithm string to JwtAlgorithm.
fn algorithm_str_to_jwt_algorithm(alg: &str) -> Result<super::jwt::JwtAlgorithm, JwksError> {
    match alg {
        "EdDSA" => Ok(super::jwt::JwtAlgorithm::EdDSA),
        "RS256" => Ok(super::jwt::JwtAlgorithm::RS256),
        "ES256" => Ok(super::jwt::JwtAlgorithm::ES256),
        other => Err(JwksError::UnsupportedAlgorithm(other.into())),
    }
}

/// Decode base64url to bytes.
fn base64url_decode(input: &str) -> Result<Vec<u8>, String> {
    URL_SAFE_NO_PAD
        .decode(input)
        .map_err(|e| format!("base64url decode failed: {e}"))
}

/// Extract issuer hint from a JWKS URL (e.g. `https://auth.example.com/.well-known/jwks.json`
/// → `https://auth.example.com`).
fn extract_issuer_from_url(url: &str) -> Option<String> {
    url.split("/.well-known")
        .nth(1)
        .map(|_| url.split("/.well-known").next().unwrap())
        .map(String::from)
        .filter(|s| !s.is_empty())
}

/// JWKS-related errors.
#[derive(Debug)]
pub enum JwksError {
    /// HTTP request to fetch JWKS failed.
    JwksFetchFailed(String),
    /// JSON parsing of JWKS failed.
    JwksParseError(String),
    /// Requested key ID not found.
    KeyNotFound(String),
    /// Key type not recognized (not OKP/RSA/EC).
    UnsupportedKeyType(String),
    /// Algorithm not supported.
    UnsupportedAlgorithm(String),
    /// Required key data missing (e.g. "x" for Ed25519).
    MissingKeyData(String),
    /// Failed to parse key material.
    KeyParseError(String),
    /// Wraps a JwtError from verifier creation.
    Jwt(JwtError),
}

impl core::fmt::Display for JwksError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            JwksError::JwksFetchFailed(s) => write!(f, "JWKS fetch failed: {s}"),
            JwksError::JwksParseError(s) => write!(f, "JWKS parse error: {s}"),
            JwksError::KeyNotFound(kid) => write!(f, "JWK not found: {kid}"),
            JwksError::UnsupportedKeyType(t) => write!(f, "Unsupported key type: {t}"),
            JwksError::UnsupportedAlgorithm(a) => write!(f, "Unsupported algorithm: {a}"),
            JwksError::MissingKeyData(field) => {
                write!(f, "Missing required JWK field: {field}")
            }
            JwksError::KeyParseError(s) => write!(f, "Key parse error: {s}"),
            JwksError::Jwt(e) => write!(f, "JWT error: {e}"),
        }
    }
}

impl std::error::Error for JwksError {}

impl From<JwtError> for JwksError {
    fn from(e: JwtError) -> Self {
        JwksError::Jwt(e)
    }
}

// ===========================================================================
// HTTP fetch — uses the cross-platform HttpClient trait (native + wasm).
// ===========================================================================

async fn fetch_jwks(url: &str) -> Result<String, JwksError> {
    use foundation_core::url::Uri;
    use foundation_netio::http::default_http_client;
    use foundation_netio::shared::client::request::PreparedRequest;
    use foundation_netio::shared::http::{
        SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod,
    };

    let client = default_http_client();
    let uri = Uri::parse(url)
        .map_err(|e| JwksError::JwksFetchFailed(format!("invalid URL: {e}")))?;
    let mut headers = SimpleHeaders::new();
    headers.insert(SimpleHeader::ACCEPT, vec!["application/json".into()]);

    let req = PreparedRequest {
        method: SimpleMethod::GET,
        url: uri,
        headers,
        body: SendSafeBody::None,
        extensions: Default::default(),
    };

    let resp = client
        .send_async(req)
        .await
        .map_err(|e| JwksError::JwksFetchFailed(e.to_string()))?;

    let status: usize = resp.get_status().into();
    if !(200..300).contains(&status) {
        let body = match resp.get_body_ref() {
            SendSafeBody::Text(t) => t.clone(),
            SendSafeBody::Bytes(b) => String::from_utf8_lossy(b).to_string(),
            _ => String::new(),
        };
        return Err(JwksError::JwksFetchFailed(format!(
            "HTTP {status}: {body}"
        )));
    }

    match resp.get_body_ref() {
        SendSafeBody::Text(t) => Ok(t.clone()),
        SendSafeBody::Bytes(b) => {
            String::from_utf8(b.clone()).map_err(|e| JwksError::JwksFetchFailed(e.to_string()))
        }
        _ => Ok(String::new()),
    }
}
