//! ACME (RFC 8555) certificate provisioning — Let's Encrypt DNS-01.
//!
//! Full protocol flow via our shared HTTP client:
//! 1. Fetch directory → get endpoints
//! 2. Create account (newAccount) with JWS-signed request
//! 3. Place order (newOrder) for requested domains
//! 4. Solve DNS-01 challenges (create TXT records, verify, clean up)
//! 5. Finalize order with CSR → get certificate
//!
//! DNS-01 challenges are set via a caller-provided callback (for a
//! Cloudflare-hosted zone, one that calls the Cloudflare DNS API).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use foundation_context::Ctx;
use foundation_core::valtron::sleep_async;
use foundation_netio::shared::client::body_reader::{collect_bytes_async, AsyncSendSafeBody};
use foundation_netio::shared::client::request::PreparedRequest;
use foundation_netio::shared::http::{SimpleHeader, SimpleHeaders};
use foundation_netio::{DynNetClient, PreparedRequestBuilder};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── Constants ──────────────────────────────────────────────────────────

/// Let's Encrypt staging directory URL (untrusted certs, high rate limits).
pub const ACME_STAGING: &str = "https://acme-staging-v02.api.letsencrypt.org/directory";
/// Let's Encrypt production directory URL.
pub const ACME_PRODUCTION: &str = "https://acme-v02.api.letsencrypt.org/directory";

/// DNS-01 challenge callback: (domain, `token_value`) → Result
// The callback shape is the public alias; a named struct would not add clarity.
#[allow(clippy::type_complexity)]
pub type Dns01Setter = Arc<dyn Fn(&str, &str) -> Result<(), String> + Send + Sync>;

// ── ACME API types ─────────────────────────────────────────────────────

// The field names mirror the ACME directory JSON keys.
#[allow(clippy::struct_field_names)]
#[derive(Debug, Deserialize)]
struct Directory {
    #[serde(rename = "newAccount")]
    new_account: String,
    #[serde(rename = "newOrder")]
    new_order: String,
    #[serde(rename = "newNonce")]
    new_nonce: String,
}

#[derive(Debug, Serialize)]
struct NewAccountRequest {
    #[serde(rename = "termsOfServiceAgreed")]
    terms_of_service_agreed: bool,
    contact: Vec<String>,
}

#[derive(Debug, Serialize)]
struct NewOrderRequest {
    identifiers: Vec<Identifier>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Identifier {
    #[serde(rename = "type")]
    typ: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct AuthorizationResponse {
    identifier: IdentifierObj,
    challenges: Vec<ChallengeResponse>,
}

#[derive(Debug, Deserialize)]
struct IdentifierObj {
    value: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ChallengeResponse {
    #[serde(rename = "type")]
    typ: String,
    url: String,
    token: String,
}

#[derive(Debug, Serialize)]
struct FinalizeRequest {
    csr: String,
}

// ── JWS Flattened JSON Serialization ───────────────────────────────────

/// Build a JWS Flattened JSON object for ACME API authentication.
/// `url` is the ACME endpoint, `nonce` from Replay-Nonce header,
/// `payload` is the request body as JSON text (empty for POST-as-GET); this
/// encodes it as base64url.
#[must_use]
pub fn build_jws(
    url: &str,
    nonce: &str,
    payload: &str,
    key_id: Option<&str>,
    jwk: Option<&Value>,
) -> Value {
    let protected = if let Some(kid) = key_id {
        serde_json::json!({
            "alg": "ES256",
            "kid": kid,
            "nonce": nonce,
            "url": url,
        })
    } else if let Some(j) = jwk {
        serde_json::json!({
            "alg": "ES256",
            "jwk": j,
            "nonce": nonce,
            "url": url,
        })
    } else {
        serde_json::json!({
            "alg": "ES256",
            "nonce": nonce,
            "url": url,
        })
    };

    let protected_b64 = base64_url(&serde_json::to_string(&protected).unwrap_or_default());
    let payload_b64 = if payload.is_empty() {
        String::new()
    } else {
        base64_url(payload)
    };

    // Signing: the caller provides a signature callback that takes
    // `protected_b64.payload_b64` and returns the base64url-encoded signature.
    serde_json::json!({
        "protected": protected_b64,
        "payload": payload_b64,
        "signature": "",
    })
}

#[must_use]
pub fn base64_url(input: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(input.as_bytes())
}

// ── ACME Client ─────────────────────────────────────────────────────────

/// Default for [`AcmeClient::with_max_error_body`]: 64 KiB, the amount Go
/// `net/http` drains from a response body it discards.
pub const DEFAULT_MAX_ERROR_BODY: usize = 64 * 1024;

/// Default for [`AcmeClient::with_dns_wait_timeout`]: 5 s. The DNS TXT record
/// gets this long to propagate before the challenge is answered.
pub const DEFAULT_DNS_WAIT_TIMEOUT: Duration = Duration::from_secs(5);
/// Default for [`AcmeClient::with_dns_wait_interval`]: 250 ms. The DNS wait
/// sleeps in steps this long, so it yields to the executor instead of blocking.
pub const DEFAULT_DNS_WAIT_INTERVAL: Duration = Duration::from_millis(250);

/// Account state carried across requests: the account key ID (kid) from the
/// newAccount `Location` answer, and the latest `Replay-Nonce` from an answer.
#[derive(Default)]
struct Session {
    key_id: Option<String>,
    nonce: Option<String>,
}

/// ACME certificate provisioning client using the proxy's shared HTTP client.
// The signer callback type is part of the public builder surface.
#[allow(clippy::type_complexity)]
pub struct AcmeClient {
    client: DynNetClient,
    /// Most bytes of an error answer read for its message (see
    /// [`Self::with_max_error_body`]).
    max_error_body: usize,
    directory_url: String,
    /// Account state changed across `async fn` calls (`provision` takes
    /// `&self`), so it lives behind a mutex. The lock is never held across an
    /// `.await` point.
    session: Mutex<Session>,
    jwk: Option<serde_json::Value>,
    /// Signature callback: (protected.payload) → base64url signature
    signer: Option<Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>>,
    /// DNS-01 TXT record setter
    dns_setter: Option<Dns01Setter>,
    /// Longest the DNS wait sleeps in total before the challenge is answered.
    dns_wait_timeout: Duration,
    /// Longest one DNS wait sleep step may last.
    dns_wait_interval: Duration,
    /// The context whose clock the DNS wait deadline reads.
    ctx: Ctx,
}

impl AcmeClient {
    /// Create a client targeting Let's Encrypt production.
    ///
    /// `ctx` supplies the clock that times the DNS-01 propagation wait.
    pub fn production(client: DynNetClient, ctx: Ctx) -> Self {
        Self {
            client,
            max_error_body: DEFAULT_MAX_ERROR_BODY,
            directory_url: ACME_PRODUCTION.to_string(),
            session: Mutex::new(Session::default()),
            jwk: None,
            signer: None,
            dns_setter: None,
            dns_wait_timeout: DEFAULT_DNS_WAIT_TIMEOUT,
            dns_wait_interval: DEFAULT_DNS_WAIT_INTERVAL,
            ctx,
        }
    }

    /// Create a client targeting Let's Encrypt staging (for testing).
    pub fn staging(client: DynNetClient, ctx: Ctx) -> Self {
        Self {
            client,
            max_error_body: DEFAULT_MAX_ERROR_BODY,
            directory_url: ACME_STAGING.to_string(),
            session: Mutex::new(Session::default()),
            jwk: None,
            signer: None,
            dns_setter: None,
            dns_wait_timeout: DEFAULT_DNS_WAIT_TIMEOUT,
            dns_wait_interval: DEFAULT_DNS_WAIT_INTERVAL,
            ctx,
        }
    }

    /// Create a client targeting an explicit directory URL (a private/enterprise
    /// ACME CA, or a local test server such as Pebble / the mock in the F15
    /// integration test).
    pub fn with_directory(
        client: DynNetClient,
        directory_url: impl Into<String>,
        ctx: Ctx,
    ) -> Self {
        Self {
            client,
            max_error_body: DEFAULT_MAX_ERROR_BODY,
            directory_url: directory_url.into(),
            session: Mutex::new(Session::default()),
            jwk: None,
            signer: None,
            dns_setter: None,
            dns_wait_timeout: DEFAULT_DNS_WAIT_TIMEOUT,
            dns_wait_interval: DEFAULT_DNS_WAIT_INTERVAL,
            ctx,
        }
    }

    /// Set the account JWK for newAccount requests.
    #[must_use]
    pub fn with_jwk(mut self, jwk: serde_json::Value) -> Self {
        self.jwk = Some(jwk);
        self
    }

    /// Set the account key ID for authenticated requests (after account creation).
    ///
    /// ACME normally learns this from the newAccount `Location` answer; use
    /// this when the caller already has an account URL.
    #[must_use]
    ///
    /// # Panics
    ///
    /// Panics if the session lock is poisoned.
    pub fn with_key_id(mut self, kid: String) -> Self {
        self.session.get_mut().expect("session lock").key_id = Some(kid);
        self
    }

    /// Set how long the DNS wait sleeps in total before a challenge is
    /// answered. Default [`DEFAULT_DNS_WAIT_TIMEOUT`].
    #[must_use]
    pub fn with_dns_wait_timeout(mut self, timeout: Duration) -> Self {
        self.dns_wait_timeout = timeout;
        self
    }

    /// Set the longest one DNS wait sleep step. Default
    /// [`DEFAULT_DNS_WAIT_INTERVAL`]. The wait yields to the executor once per
    /// step.
    #[must_use]
    pub fn with_dns_wait_interval(mut self, interval: Duration) -> Self {
        self.dns_wait_interval = interval;
        self
    }

    /// Set the JWS signature callback.
    #[must_use]
    pub fn with_signer(
        mut self,
        signer: impl Fn(&str) -> Result<String, String> + Send + Sync + 'static,
    ) -> Self {
        self.signer = Some(Arc::new(signer));
        self
    }

    /// Set the DNS-01 TXT record callback.
    #[must_use]
    pub fn with_dns_setter(
        mut self,
        setter: impl Fn(&str, &str) -> Result<(), String> + Send + Sync + 'static,
    ) -> Self {
        self.dns_setter = Some(Arc::new(setter));
        self
    }

    /// Fetch the ACME directory and create an account. Returns the account URL
    /// from the answer's `Location` header (RFC 8555 §7.3) and stores it as
    /// the key ID (kid) for later requests.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn create_account(&self, email: &str) -> Result<String, String> {
        let dir = self.fetch_directory().await?;
        let nonce = match self.take_nonce() {
            Some(nonce) => nonce,
            None => self.fetch_nonce(&dir.new_nonce).await?,
        };

        let payload = serde_json::to_string(&NewAccountRequest {
            terms_of_service_agreed: true,
            contact: vec![format!("mailto:{email}")],
        })
        .map_err(|e| format!("serialize account: {e}"))?;

        // newAccount is signed with the JWK: the kid only exists after this
        // call returns the account URL in `Location`.
        let jws = build_jws(&dir.new_account, &nonce, &payload, None, self.jwk.as_ref());
        let signed = self.sign_jws(&jws)?;
        let request = jose_post(&dir.new_account, &signed)?;
        let response = self
            .client
            .send_async(request)
            .await
            .map_err(|e| format!("send {}: {e}", dir.new_account))?;
        let (status, headers, body) = response.into_async_parts();
        let code: usize = status.into();
        self.remember_nonce(&headers);
        if !(200..=299).contains(&code) {
            let message = read_bounded(body, self.max_error_body).await;
            return Err(format!("HTTP {code} from {}: {message}", dir.new_account));
        }

        // RFC 8555 §7.3: the account URL is the `Location` header of the 201
        // answer.
        let account_url = headers
            .get(&SimpleHeader::LOCATION)
            .and_then(|vals| vals.first())
            .cloned()
            .ok_or_else(|| "no Location header on newAccount answer".to_string())?;
        self.set_kid(account_url.clone());
        let _ = read_all(&dir.new_account, body).await?;

        Ok(account_url)
    }

    /// Place an order for the given domains.
    /// Returns `(finalize_url, authorization_urls)`.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn place_order(&self, domains: &[String]) -> Result<(String, Vec<String>), String> {
        let dir = self.fetch_directory().await?;

        let identifiers: Vec<Identifier> = domains
            .iter()
            .map(|d| Identifier {
                typ: "dns".to_string(),
                value: d.clone(),
            })
            .collect();

        let payload = serde_json::to_string(&NewOrderRequest { identifiers })
            .map_err(|e| format!("serialize order: {e}"))?;
        let (_headers, bytes) = self.post_signed(&dir.new_order, &payload).await?;
        let resp: Value = serde_json::from_slice(&bytes)
            .map_err(|e| format!("parse JSON from {}: {e}", dir.new_order))?;

        let finalize_url = resp
            .get("finalize")
            .and_then(|f| f.as_str())
            .map(String::from)
            .ok_or_else(|| "no finalize URL in order response".to_string())?;

        let authorizations: Vec<String> = resp
            .get("authorizations")
            .and_then(|a| a.as_array())
            .map_or_default(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            });

        Ok((finalize_url, authorizations))
    }

    /// Fetch an authorization and return its DNS-01 challenge.
    async fn get_dns01_challenge(&self, auth_url: &str) -> Result<ChallengeResponse, String> {
        let (_headers, bytes) = self.post_signed(auth_url, "").await?;
        let resp: AuthorizationResponse = serde_json::from_slice(&bytes)
            .map_err(|e| format!("parse JSON from {auth_url}: {e}"))?;

        resp.challenges
            .into_iter()
            .find(|c| c.typ == "dns-01")
            .ok_or_else(|| "no dns-01 challenge found".to_string())
    }

    /// Respond to a challenge (signal ready for validation).
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn respond_to_challenge(&self, challenge_url: &str) -> Result<(), String> {
        self.post_signed(challenge_url, "{}").await?;
        Ok(())
    }

    /// Finalize an order with a CSR (DER-encoded PKCS#10, base64url).
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn finalize_order(
        &self,
        finalize_url: &str,
        csr_der: &[u8],
    ) -> Result<String, String> {
        let csr_b64 =
            base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, csr_der);
        let payload = serde_json::to_string(&FinalizeRequest { csr: csr_b64 })
            .map_err(|e| format!("serialize finalize: {e}"))?;
        let (_headers, bytes) = self.post_signed(finalize_url, &payload).await?;
        let resp: Value = serde_json::from_slice(&bytes)
            .map_err(|e| format!("parse JSON from {finalize_url}: {e}"))?;

        resp.get("certificate")
            .and_then(|c| c.as_str())
            .map(String::from)
            .ok_or_else(|| "no certificate URL in response".to_string())
    }

    /// Download the certificate chain from the certificate URL (PEM text).
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn download_certificate(&self, cert_url: &str) -> Result<String, String> {
        let (_headers, bytes) = self.post_signed(cert_url, "").await?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// Full automated flow: order → challenge → validate → finalize → download.
    ///
    /// `csr_der` is a DER-encoded PKCS#10 certificate signing request.
    /// Returns `(certificate_pem, raw_cert_bytes)`.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails.
    pub async fn provision(
        &self,
        domains: &[String],
        email: &str,
        csr_der: &[u8],
    ) -> Result<(String, Vec<u8>), String> {
        // 1. Create account
        let _account_url = self.create_account(email).await?;

        // 2. Place order — capture finalize_url from first call.
        let (finalize_url, authorizations) = self.place_order(domains).await?;

        // 3. Solve DNS-01 challenges.
        let thumbprint = Self::account_thumbprint(
            &self
                .jwk
                .as_ref()
                .map_or_default(std::string::ToString::to_string),
        );
        let dns = self.dns_setter.as_ref().ok_or("no DNS setter configured")?;

        for auth_url in &authorizations {
            let challenge = self.get_dns01_challenge(auth_url).await?;
            let dns_value = Self::dns01_value(&challenge.token, &thumbprint);
            let domain = self.get_authorization_domain(auth_url).await?;

            let record_name = format!("_acme-challenge.{domain}");
            dns(&record_name, &dns_value)?;

            // Let the TXT record propagate before answering the challenge. The
            // wait yields to the executor in `dns_wait_interval` steps until
            // `dns_wait_timeout` elapses, so no worker thread is blocked.
            let deadline = self.ctx.clock().now() + self.dns_wait_timeout;
            loop {
                let now = self.ctx.clock().now();
                if now >= deadline {
                    break;
                }
                let step = (deadline - now).min(self.dns_wait_interval);
                sleep_async(step, &self.ctx).await;
            }
            self.respond_to_challenge(&challenge.url).await?;
        }

        // 4. Finalize order with CSR.
        let cert_url = self.finalize_order(&finalize_url, csr_der).await?;

        // 5. Download certificate (raw bytes).
        let (_headers, raw_cert) = self.post_signed(&cert_url, "").await?;

        // 6. Also get the cert as text (for PEM output).
        let cert_pem = String::from_utf8(raw_cert.clone())
            .unwrap_or_else(|_| "certificate downloaded".to_string());

        Ok((cert_pem, raw_cert))
    }

    /// Read at most `bytes` of an error answer (a non-2xx status) for the
    /// error message; the rest is never read. Default
    /// [`DEFAULT_MAX_ERROR_BODY`].
    #[must_use]
    pub fn with_max_error_body(mut self, bytes: usize) -> Self {
        self.max_error_body = bytes;
        self
    }

    /// Compute the DNS-01 TXT record value.
    #[must_use]
    pub fn dns01_value(token: &str, account_thumbprint: &str) -> String {
        use sha2::{Digest, Sha256};
        let key_auth = format!("{token}.{account_thumbprint}");
        let hash = Sha256::digest(key_auth.as_bytes());
        base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, hash)
    }

    /// Compute the account key thumbprint (JWS `jwk` → SHA-256 → base64url).
    #[must_use]
    pub fn account_thumbprint(jwk_json: &str) -> String {
        use sha2::{Digest, Sha256};
        let hash = Sha256::digest(jwk_json.as_bytes());
        base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, hash)
    }

    // ── Internal helpers ────────────────────────────────────────────────

    async fn fetch_directory(&self) -> Result<Directory, String> {
        let resp = self.get_json(&self.directory_url).await?;
        serde_json::from_value(resp).map_err(|e| format!("parse directory: {e}"))
    }

    async fn fetch_nonce(&self, new_nonce_url: &str) -> Result<String, String> {
        // RFC 8555 §7.2: HEAD to newNonce endpoint, nonce in Replay-Nonce header.
        let request = PreparedRequestBuilder::head(new_nonce_url)
            .map_err(|e| format!("build HEAD {new_nonce_url}: {e}"))?
            .build();
        let (headers, _body) = self.send(new_nonce_url, request).await?;

        let nonce_key = SimpleHeader::custom("replay-nonce");
        headers
            .get(&nonce_key)
            .and_then(|vals| vals.first())
            .cloned()
            .ok_or_else(|| format!("no replay-nonce header from {new_nonce_url}"))
    }

    async fn fetch_nonce_nopost(&self) -> Result<String, String> {
        let dir = self.fetch_directory().await?;
        self.fetch_nonce(&dir.new_nonce).await
    }

    async fn get_json(&self, url: &str) -> Result<Value, String> {
        let request = PreparedRequestBuilder::get(url)
            .map_err(|e| format!("build GET {url}: {e}"))?
            .build();
        let (_, body) = self.send(url, request).await?;
        let bytes = read_all(url, body).await?;
        serde_json::from_slice(&bytes).map_err(|e| format!("parse JSON from {url}: {e}"))
    }

    /// POST a signed JWS to `url` with `payload` as the JWS payload (JSON text,
    /// or "" for a POST-as-GET). The nonce is taken from the carried
    /// `Replay-Nonce`; a fresh one is fetched only when none is carried. A
    /// `badNonce` answer carries a fresh nonce, and the request is retried
    /// once with it. Returns the answer headers and the whole body.
    async fn post_signed(
        &self,
        url: &str,
        payload: &str,
    ) -> Result<(SimpleHeaders, Vec<u8>), String> {
        let mut attempts = 0;
        loop {
            let nonce = match self.take_nonce() {
                Some(nonce) => nonce,
                None => self.fetch_nonce_nopost().await?,
            };

            let jws = build_jws(
                url,
                &nonce,
                payload,
                self.kid().as_deref(),
                self.jwk.as_ref(),
            );
            let signed = self.sign_jws(&jws)?;
            let request = jose_post(url, &signed)?;
            let response = self
                .client
                .send_async(request)
                .await
                .map_err(|e| format!("send {url}: {e}"))?;
            let (status, headers, body) = response.into_async_parts();
            let code: usize = status.into();
            self.remember_nonce(&headers);

            if (200..=299).contains(&code) {
                let bytes = read_all(url, body).await?;
                return Ok((headers, bytes));
            }

            let message = read_bounded(body, self.max_error_body).await;
            if attempts == 0 && is_bad_nonce(&message) {
                attempts += 1;
                continue;
            }
            return Err(format!("HTTP {code} from {url}: {message}"));
        }
    }

    /// The account key ID (kid) from the newAccount `Location` answer, if set.
    fn kid(&self) -> Option<String> {
        self.session
            .lock()
            .expect("acme session lock")
            .key_id
            .clone()
    }

    /// Store the account key ID (kid).
    fn set_kid(&self, kid: String) {
        self.session.lock().expect("acme session lock").key_id = Some(kid);
    }

    /// Take and clear the carried `Replay-Nonce`, if any.
    fn take_nonce(&self) -> Option<String> {
        self.session.lock().expect("acme session lock").nonce.take()
    }

    /// Carry the `Replay-Nonce` from an answer for the next request.
    fn remember_nonce(&self, headers: &SimpleHeaders) {
        let nonce_key = SimpleHeader::custom("replay-nonce");
        if let Some(nonce) = headers.get(&nonce_key).and_then(|vals| vals.first()) {
            self.session.lock().expect("acme session lock").nonce = Some(nonce.clone());
        }
    }

    /// Send `request`; an error unless the status is 2xx. The body of an
    /// error answer is read only up to `max_error_body`, for the message.
    async fn send(
        &self,
        url: &str,
        request: PreparedRequest,
    ) -> Result<(SimpleHeaders, AsyncSendSafeBody), String> {
        let response = self
            .client
            .send_async(request)
            .await
            .map_err(|e| format!("send {url}: {e}"))?;
        let (status, headers, body) = response.into_async_parts();
        let code: usize = status.into();
        if !(200..=299).contains(&code) {
            let message = read_bounded(body, self.max_error_body).await;
            return Err(format!("HTTP {code} from {url}: {message}"));
        }
        Ok((headers, body))
    }

    async fn get_authorization_domain(&self, auth_url: &str) -> Result<String, String> {
        let (_headers, bytes) = self.post_signed(auth_url, "").await?;
        let resp: AuthorizationResponse = serde_json::from_slice(&bytes)
            .map_err(|e| format!("parse JSON from {auth_url}: {e}"))?;
        Ok(resp.identifier.value)
    }

    #[allow(clippy::similar_names)]
    fn sign_jws(&self, jws: &Value) -> Result<Value, String> {
        let signer = self.signer.as_ref().ok_or("no signer configured")?;
        let protected = jws["protected"].as_str().unwrap_or("");
        let payload = jws["payload"].as_str().unwrap_or("");
        let signing_input = format!("{protected}.{payload}");
        let signature = signer(&signing_input)?;
        let mut signed = jws.clone();
        signed["signature"] = serde_json::Value::String(signature);
        Ok(signed)
    }
}

// ── Module helpers ──────────────────────────────────────────────────────

/// A `POST` of `body` as `application/jose+json` (RFC 8555 §6.2).
fn jose_post(url: &str, body: &Value) -> Result<PreparedRequest, String> {
    let json = serde_json::to_vec(body).map_err(|e| format!("serialize: {e}"))?;
    Ok(PreparedRequestBuilder::post(url)
        .map_err(|e| format!("build POST {url}: {e}"))?
        .header(SimpleHeader::CONTENT_TYPE, "application/jose+json")
        .body_bytes(json)
        .build())
}

/// True when an ACME problem answer is `urn:ietf:params:acme:error:badNonce`.
fn is_bad_nonce(message: &str) -> bool {
    if let Ok(problem) = serde_json::from_str::<Value>(message) {
        return problem
            .get("type")
            .and_then(|t| t.as_str())
            .is_some_and(|t| t.ends_with(":badNonce"));
    }
    message.contains("badNonce")
}

/// Read a 2xx body whole: ACME answers are small JSON documents or a PEM
/// chain that the next step of the flow needs entire.
async fn read_all(url: &str, body: AsyncSendSafeBody) -> Result<Vec<u8>, String> {
    collect_bytes_async(body)
        .await
        .map_err(|e| format!("read body from {url}: {e}"))
}

/// Read at most `limit` bytes of an error body as lossy UTF-8.
async fn read_bounded(mut body: AsyncSendSafeBody, limit: usize) -> String {
    use futures_lite::StreamExt;

    let mut buf = Vec::new();
    while let Some(next) = body.next().await {
        match next {
            Ok(chunk) => {
                let room = limit - buf.len();
                if chunk.len() > room {
                    buf.extend_from_slice(&chunk[..room]);
                    buf.extend_from_slice(b" (truncated)");
                    break;
                }
                buf.extend_from_slice(&chunk);
            }
            Err(e) => {
                buf.extend_from_slice(format!(" (body read error: {e})").as_bytes());
                break;
            }
        }
    }
    String::from_utf8_lossy(&buf).into_owned()
}
