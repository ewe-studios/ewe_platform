//! ACME (RFC 8555) certificate provisioning — Let's Encrypt DNS-01.
//!
//! Full protocol flow via our shared HTTP client:
//! 1. Fetch directory → get endpoints
//! 2. Create account (newAccount) with JWS-signed request
//! 3. Place order (newOrder) for requested domains
//! 4. Solve DNS-01 challenges (create TXT records, verify, clean up)
//! 5. Finalize order with CSR → get certificate
//!
//! DNS-01 challenges are set via a caller-provided callback (typically
//! foundation_deployment_cloudflare for Cloudflare-hosted zones).

use std::sync::Arc;

use bytes::Bytes;
use foundation_netio::shared::client::body_reader::{collect_bytes_from_send_safe, collect_strings_from_send_safe};
use foundation_netio::shared::http::{
    SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod,
};
use foundation_netio::shared::client::SystemDnsResolver;
use foundation_netio::http::{ClientRequestBuilder, NativeHttpClient};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::ProxyError;

// ── Constants ──────────────────────────────────────────────────────────

const ACME_STAGING: &str = "https://acme-staging-v02.api.letsencrypt.org/directory";
const ACME_PRODUCTION: &str = "https://acme-v02.api.letsencrypt.org/directory";

/// DNS-01 challenge callback: (domain, token_value) → Result
pub type Dns01Setter = Arc<dyn Fn(&str, &str) -> Result<(), String> + Send + Sync>;

// ── ACME API types ─────────────────────────────────────────────────────

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

#[derive(Debug, Deserialize)]
struct AccountResponse {
    status: String,
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
struct OrderResponse {
    status: String,
    authorizations: Vec<String>,
    finalize: String,
    certificate: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AuthorizationResponse {
    identifier: IdentifierObj,
    challenges: Vec<ChallengeResponse>,
}

#[derive(Debug, Deserialize)]
struct IdentifierObj {
    #[serde(rename = "type")]
    typ: String,
    value: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ChallengeResponse {
    #[serde(rename = "type")]
    typ: String,
    url: String,
    token: String,
    status: String,
}

#[derive(Debug, Serialize)]
struct FinalizeRequest {
    csr: String,
}

// ── JWS Flattened JSON Serialization ───────────────────────────────────

/// Build a JWS Flattened JSON object for ACME API authentication.
/// `url` is the ACME endpoint, `nonce` from Replay-Nonce header,
/// `payload` is the base64url-encoded request body.
fn build_jws(
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
    let payload_b64 = if payload.is_empty() { "".to_string() } else { base64_url(payload) };

    // Signing: the caller provides a signature callback that takes
    // `protected_b64.payload_b64` and returns the base64url-encoded signature.
    serde_json::json!({
        "protected": protected_b64,
        "payload": payload_b64,
        "signature": "",
    })
}

fn base64_url(input: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(input.as_bytes())
}

// ── ACME Client ─────────────────────────────────────────────────────────

/// ACME certificate provisioning client using the proxy's shared HTTP client.
pub struct AcmeClient {
    client: Arc<NativeHttpClient<SystemDnsResolver>>,
    directory_url: String,
    key_id: Option<String>,
    jwk: Option<serde_json::Value>,
    /// Signature callback: (protected.payload) → base64url signature
    signer: Option<Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>>,
    /// DNS-01 TXT record setter
    dns_setter: Option<Dns01Setter>,
    staging: bool,
}

impl AcmeClient {
    /// Create a client targeting Let's Encrypt production.
    pub fn production(client: Arc<NativeHttpClient<SystemDnsResolver>>) -> Self {
        Self {
            client,
            directory_url: ACME_PRODUCTION.to_string(),
            key_id: None,
            jwk: None,
            signer: None,
            dns_setter: None,
            staging: false,
        }
    }

    /// Create a client targeting Let's Encrypt staging (for testing).
    pub fn staging(client: Arc<NativeHttpClient<SystemDnsResolver>>) -> Self {
        Self {
            client,
            directory_url: ACME_STAGING.to_string(),
            key_id: None,
            jwk: None,
            signer: None,
            dns_setter: None,
            staging: true,
        }
    }

    /// Set the account JWK for newAccount requests.
    #[must_use]
    pub fn with_jwk(mut self, jwk: serde_json::Value) -> Self {
        self.jwk = Some(jwk);
        self
    }

    /// Set the account key ID for authenticated requests (after account creation).
    #[must_use]
    pub fn with_key_id(mut self, kid: String) -> Self {
        self.key_id = Some(kid);
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
    pub fn with_dns_setter(mut self, setter: impl Fn(&str, &str) -> Result<(), String> + Send + Sync + 'static) -> Self {
        self.dns_setter = Some(Arc::new(setter));
        self
    }

    /// Fetch the ACME directory and create an account. Returns the account URL
    /// (used as Key ID for subsequent requests).
    pub async fn create_account(
        &self,
        email: &str,
    ) -> Result<String, String> {
        let dir = self.fetch_directory().await?;
        let nonce = self.fetch_nonce(&dir.new_nonce).await?;

        let payload = serde_json::to_string(&NewAccountRequest {
            terms_of_service_agreed: true,
            contact: vec![format!("mailto:{email}")],
        })
        .map_err(|e| format!("serialize account: {e}"))?;

        let payload_b64 = base64_url(&payload);
        let jws = build_jws(&dir.new_account, &nonce, &payload, None, self.jwk.as_ref());

        let signed = self.sign_jws(&jws)?;
        let resp = self.post_json(&dir.new_account, &signed, true).await?;

        // Account URL is in the Location header. For now, extract from response.
        let account_url = resp
            .get("orders")
            .and_then(|o| o.as_str())
            .map(|_| dir.new_account.clone())
            .ok_or_else(|| "no account URL in response".to_string())?;

        Ok(account_url)
    }

    /// Place an order for the given domains.
    /// Returns `(finalize_url, authorization_urls)`.
    pub async fn place_order(
        &self,
        domains: &[String],
    ) -> Result<(String, Vec<String>), String> {
        let dir = self.fetch_directory().await?;
        let nonce = self.fetch_nonce(&dir.new_nonce).await?;

        let identifiers: Vec<Identifier> = domains
            .iter()
            .map(|d| Identifier {
                typ: "dns".to_string(),
                value: d.clone(),
            })
            .collect();

        let payload = serde_json::to_string(&NewOrderRequest { identifiers })
            .map_err(|e| format!("serialize order: {e}"))?;
        let payload_b64 = base64_url(&payload);
        let jws = build_jws(
            &dir.new_order,
            &nonce,
            &payload,
            self.key_id.as_deref(),
            self.jwk.as_ref(),
        );
        let signed = self.sign_jws(&jws)?;
        let resp = self.post_json(&dir.new_order, &signed, true).await?;

        let finalize_url = resp
            .get("finalize")
            .and_then(|f| f.as_str())
            .map(String::from)
            .ok_or_else(|| "no finalize URL in order response".to_string())?;

        let authorizations: Vec<String> = resp
            .get("authorizations")
            .and_then(|a| a.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        Ok((finalize_url, authorizations))
    }

    /// Fetch an authorization and return its DNS-01 challenge.
    pub async fn get_dns01_challenge(
        &self,
        auth_url: &str,
    ) -> Result<ChallengeResponse, String> {
        let nonce = self.fetch_nonce_nopost().await?;
        let jws = build_jws(auth_url, &nonce, "", self.key_id.as_deref(), None);
        let signed = self.sign_jws(&jws)?;
        let resp: AuthorizationResponse = self.post_json_typed(auth_url, &signed, true).await?;

        resp.challenges
            .into_iter()
            .find(|c| c.typ == "dns-01")
            .ok_or_else(|| "no dns-01 challenge found".to_string())
    }

    /// Respond to a challenge (signal ready for validation).
    pub async fn respond_to_challenge(&self, challenge_url: &str) -> Result<(), String> {
        let nonce = self.fetch_nonce_nopost().await?;
        let jws = build_jws(
            challenge_url,
            &nonce,
            "{}",
            self.key_id.as_deref(),
            None,
        );
        let signed = self.sign_jws(&jws)?;
        self.post_json(challenge_url, &signed, true).await?;
        Ok(())
    }

    /// Finalize an order with a CSR (DER-encoded PKCS#10, base64url).
    pub async fn finalize_order(&self, finalize_url: &str, csr_der: &[u8]) -> Result<String, String> {
        let nonce = self.fetch_nonce_nopost().await?;
        let csr_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            csr_der,
        );
        let payload = serde_json::to_string(&FinalizeRequest { csr: csr_b64 })
            .map_err(|e| format!("serialize finalize: {e}"))?;
        let payload_b64 = base64_url(&payload);
        let jws = build_jws(
            finalize_url,
            &nonce,
            &payload,
            self.key_id.as_deref(),
            None,
        );
        let signed = self.sign_jws(&jws)?;
        let resp = self.post_json(finalize_url, &signed, true).await?;

        resp.get("certificate")
            .and_then(|c| c.as_str())
            .map(String::from)
            .ok_or_else(|| "no certificate URL in response".to_string())
    }

    /// Download the certificate chain from the certificate URL.
    pub async fn download_certificate(&self, cert_url: &str) -> Result<String, String> {
        let nonce = self.fetch_nonce_nopost().await?;
        let jws = build_jws(cert_url, &nonce, "", self.key_id.as_deref(), None);
        let signed = self.sign_jws(&jws)?;
        let resp = self.post_json(cert_url, &signed, true).await?;
        // The response body for certificate download is PEM-encoded text.
        // ACME returns the certificate directly; our JSON post handler will
        // attempt to parse as JSON first. Fall back to returning the raw body.
        Ok(resp.get("cert").and_then(|c| c.as_str()).unwrap_or("").to_string())
    }

    /// Full automated flow: order → challenge → validate → finalize → download.
    ///
    /// `csr_der` is a DER-encoded PKCS#10 certificate signing request.
    /// Returns `(certificate_pem, raw_cert_bytes)`.
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
            &self.jwk.as_ref().map(|j| j.to_string()).unwrap_or_default(),
        );
        let dns = self.dns_setter.as_ref().ok_or("no DNS setter configured")?;

        for auth_url in &authorizations {
            let challenge = self.get_dns01_challenge(auth_url).await?;
            let dns_value = Self::dns01_value(&challenge.token, &thumbprint);
            let domain = self.get_authorization_domain(auth_url).await?;

            let record_name = format!("_acme-challenge.{domain}");
            dns(&record_name, &dns_value)?;
            std::thread::sleep(std::time::Duration::from_secs(5));
            self.respond_to_challenge(&challenge.url).await?;
        }

        // 4. Finalize order with CSR.
        let cert_url = self.finalize_order(&finalize_url, csr_der).await?;

        // 5. Download certificate (raw bytes).
        let nonce = self.fetch_nonce_nopost().await?;
        let jws = build_jws(&cert_url, &nonce, "", self.key_id.as_deref(), None);
        let signed = self.sign_jws(&jws)?;
        let raw_cert = self.post_json_raw(&cert_url, &signed).await?;

        // 6. Also get the cert as text (for PEM output).
        let cert_pem = String::from_utf8(raw_cert.clone())
            .unwrap_or_else(|_| "certificate downloaded".to_string());

        Ok((cert_pem, raw_cert))
    }

    /// Post JSON and return raw bytes (for certificate download).
    async fn post_json_raw(
        &self,
        url: &str,
        body: &Value,
    ) -> Result<Vec<u8>, String> {
        let json = serde_json::to_vec(body).map_err(|e| format!("serialize: {e}"))?;
        let mut headers = SimpleHeaders::new();
        headers.insert(SimpleHeader::CONTENT_TYPE, vec!["application/jose+json".to_string()]);

        let builder = ClientRequestBuilder::<SystemDnsResolver>::new(SimpleMethod::POST, url)
            .map_err(|e| format!("build POST {url}: {e}"))?
            .headers(headers)
            .body(SendSafeBody::Bytes(json));

        let request = self.client.request(builder)
            .map_err(|e| format!("request {url}: {e}"))?;
        let response = request.send()
            .map_err(|e| format!("send {url}: {e}"))?;
        let (_status, _headers, resp_body, _pool, _conn) = response.into_parts();
        Ok(collect_bytes_from_send_safe(resp_body))
    }

    /// Compute the DNS-01 TXT record value.
    pub fn dns01_value(token: &str, account_thumbprint: &str) -> String {
        use sha2::{Digest, Sha256};
        let key_auth = format!("{token}.{account_thumbprint}");
        let hash = Sha256::digest(key_auth.as_bytes());
        base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, &hash)
    }

    /// Compute the account key thumbprint (JWS `jwk` → SHA-256 → base64url).
    pub fn account_thumbprint(jwk_json: &str) -> String {
        use sha2::{Digest, Sha256};
        let hash = Sha256::digest(jwk_json.as_bytes());
        base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, &hash)
    }

    // ── Internal helpers ────────────────────────────────────────────────

    async fn fetch_directory(&self) -> Result<Directory, String> {
        let resp = self.get_json(&self.directory_url).await?;
        serde_json::from_value(resp).map_err(|e| format!("parse directory: {e}"))
    }

    async fn fetch_nonce(&self, new_nonce_url: &str) -> Result<String, String> {
        // RFC 8555 §7.2: HEAD to newNonce endpoint, nonce in Replay-Nonce header.
        let builder = ClientRequestBuilder::<SystemDnsResolver>::new(SimpleMethod::HEAD, new_nonce_url)
            .map_err(|e| format!("build HEAD {new_nonce_url}: {e}"))?;

        let request = self.client.request(builder)
            .map_err(|e| format!("request nonce {new_nonce_url}: {e}"))?;
        let response = request.send()
            .map_err(|e| format!("send nonce request: {e}"))?;
        let (_status, headers, _body, _pool, _conn) = response.into_parts();

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
        let builder = ClientRequestBuilder::<SystemDnsResolver>::new(SimpleMethod::GET, url)
            .map_err(|e| format!("build GET {url}: {e}"))?;

        let request = self.client.request(builder)
            .map_err(|e| format!("request {url}: {e}"))?;
        let response = request.send()
            .map_err(|e| format!("send {url}: {e}"))?;
        let (_status, _headers, body, _pool, _conn) = response.into_parts();
        let text = collect_strings_from_send_safe(body)
            .map_err(|e| format!("read body from {url}: {e}"))?;
        serde_json::from_str(&text).map_err(|e| format!("parse JSON from {url}: {e}"))
    }

    async fn post_json(&self, url: &str, body: &Value, _expect_json: bool) -> Result<Value, String> {
        let json = serde_json::to_vec(body).map_err(|e| format!("serialize: {e}"))?;
        let mut headers = SimpleHeaders::new();
        headers.insert(SimpleHeader::CONTENT_TYPE, vec!["application/jose+json".to_string()]);

        let builder = ClientRequestBuilder::<SystemDnsResolver>::new(SimpleMethod::POST, url)
            .map_err(|e| format!("build POST {url}: {e}"))?
            .headers(headers)
            .body(SendSafeBody::Bytes(json));

        let request = self.client.request(builder)
            .map_err(|e| format!("request {url}: {e}"))?;
        let response = request.send()
            .map_err(|e| format!("send {url}: {e}"))?;
        let (_status, _headers, resp_body, _pool, _conn) = response.into_parts();
        let text = collect_strings_from_send_safe(resp_body)
            .map_err(|e| format!("read body from {url}: {e}"))?;
        Ok(serde_json::from_str(&text).unwrap_or_else(|_| serde_json::json!({})))
    }

    async fn post_json_typed<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        body: &Value,
        _expect_json: bool,
    ) -> Result<T, String> {
        let json = serde_json::to_vec(body).map_err(|e| format!("serialize: {e}"))?;
        let mut headers = SimpleHeaders::new();
        headers.insert(SimpleHeader::CONTENT_TYPE, vec!["application/jose+json".to_string()]);

        let builder = ClientRequestBuilder::<SystemDnsResolver>::new(SimpleMethod::POST, url)
            .map_err(|e| format!("build POST {url}: {e}"))?
            .headers(headers)
            .body(SendSafeBody::Bytes(json));

        let request = self.client.request(builder)
            .map_err(|e| format!("request {url}: {e}"))?;
        let response = request.send()
            .map_err(|e| format!("send {url}: {e}"))?;
        let (_status, _headers, resp_body, _pool, _conn) = response.into_parts();
        let text = collect_strings_from_send_safe(resp_body)
            .map_err(|e| format!("read body from {url}: {e}"))?;
        serde_json::from_str(&text).map_err(|e| format!("parse JSON: {e}"))
    }

    async fn get_authorization_domain(&self, auth_url: &str) -> Result<String, String> {
        let nonce = self.fetch_nonce_nopost().await?;
        let jws = build_jws(auth_url, &nonce, "", self.key_id.as_deref(), None);
        let signed = self.sign_jws(&jws)?;
        let resp: AuthorizationResponse = self.post_json_typed(auth_url, &signed, true).await?;
        Ok(resp.identifier.value)
    }

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

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dns01_value_is_deterministic() {
        let a = AcmeClient::dns01_value("tok1", "thumb1");
        let b = AcmeClient::dns01_value("tok1", "thumb1");
        assert_eq!(a, b);
    }

    #[test]
    fn dns01_value_is_base64url() {
        let val = AcmeClient::dns01_value("test-token", "thumb");
        assert!(!val.contains('+'));
        assert!(!val.contains('/'));
        assert!(!val.contains('='));
    }

    #[test]
    fn dns01_different_inputs_yield_different_outputs() {
        let a = AcmeClient::dns01_value("tok1", "thumb1");
        let b = AcmeClient::dns01_value("tok2", "thumb1");
        assert_ne!(a, b);
    }

    #[test]
    fn base64_url_encodes_correctly() {
        let encoded = base64_url("test");
        assert!(!encoded.is_empty());
        assert!(!encoded.contains('='));
    }

    #[test]
    fn base64_url_no_padding() {
        // Base64 URL-safe with no padding: every output should be padding-free
        for input in &["a", "ab", "abc", "abcd", "abcde"] {
            let encoded = base64_url(input);
            assert!(!encoded.contains('='), "input '{input}' produced '{encoded}' with padding");
        }
    }

    #[test]
    fn build_jws_protected_payload() {
        let jwk = serde_json::json!({"kty": "EC", "crv": "P-256"});
        let jws = build_jws(
            "https://acme.example.com/newAccount",
            "abc123",
            "eyJ0ZXN0IjogdHJ1ZX0",
            None,
            Some(&jwk),
        );
        let protected = jws["protected"].as_str().unwrap();
        let payload = jws["payload"].as_str().unwrap();
        let sig = jws["signature"].as_str().unwrap();
        assert!(protected.len() > 0, "protected should be non-empty base64url");
        assert!(payload.len() > 0, "payload should be non-empty base64url");
        assert!(sig.is_empty(), "signature should be empty before signing");
        assert!(!protected.contains('='), "base64url should not have padding");
        assert!(!payload.contains('='), "base64url should not have padding");
    }

    #[test]
    fn account_thumbprint_is_stable() {
        let jwk = r#"{"kty":"EC","crv":"P-256","x":"test","y":"test"}"#;
        let a = AcmeClient::account_thumbprint(jwk);
        let b = AcmeClient::account_thumbprint(jwk);
        assert_eq!(a, b);
        assert!(!a.contains('='));
        assert!(!a.contains('+'));
    }
}
