use std::sync::Arc;

use foundation_http::shared::context::ContextBag;
use foundation_http::SimpleIncomingRequest;
use serde::{Deserialize, Serialize};

use super::super::config::IdpConfig;

#[derive(Debug, Serialize)]
pub struct OidcDiscoveryDocument {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: String,
    pub jwks_uri: String,
    pub introspection_endpoint: String,
    pub device_authorization_endpoint: String,
    pub response_types_supported: Vec<String>,
    pub grant_types_supported: Vec<String>,
    pub subject_types_supported: Vec<String>,
    pub id_token_signing_alg_values_supported: Vec<String>,
    pub scopes_supported: Vec<String>,
    pub token_endpoint_auth_methods_supported: Vec<String>,
    pub code_challenge_methods_supported: Vec<String>,
}

impl OidcDiscoveryDocument {
    #[must_use]
    pub fn from_config(config: &IdpConfig) -> Self {
        let base = config.issuer_url.trim_end_matches('/');
        Self {
            issuer: config.issuer_url.clone(),
            authorization_endpoint: format!("{base}/authorize"),
            token_endpoint: format!("{base}/token"),
            userinfo_endpoint: format!("{base}/userinfo"),
            jwks_uri: format!("{base}/.well-known/jwks.json"),
            introspection_endpoint: format!("{base}/introspect"),
            device_authorization_endpoint: format!("{base}/device/authorize"),
            response_types_supported: vec!["code".into()],
            grant_types_supported: vec![
                "authorization_code".into(),
                "refresh_token".into(),
                "client_credentials".into(),
                "urn:ietf:params:oauth:grant-type:device_code".into(),
            ],
            subject_types_supported: vec!["public".into()],
            id_token_signing_alg_values_supported: vec!["EdDSA".into()],
            scopes_supported: vec!["openid".into(), "profile".into(), "email".into()],
            token_endpoint_auth_methods_supported: vec![
                "client_secret_post".into(),
                "none".into(),
            ],
            code_challenge_methods_supported: vec!["S256".into()],
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub id_token: String,
    pub token_type: String,
    pub expires_in: u64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub refresh_token: String,
    pub scope: String,
}

#[derive(Debug, Serialize)]
pub struct DeviceAuthResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u32,
}

#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    pub grant_type: String,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub code_verifier: Option<String>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
    pub device_code: Option<String>,
}

#[derive(Debug)]
pub enum IdpError {
    Internal(String),
    BadRequest(String),
    Unauthorized(String),
}

impl core::fmt::Display for IdpError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Internal(s) => write!(f, "Internal error: {s}"),
            Self::BadRequest(s) => write!(f, "Bad request: {s}"),
            Self::Unauthorized(s) => write!(f, "Unauthorized: {s}"),
        }
    }
}

impl std::error::Error for IdpError {}

pub struct IdpHandlerCore {
    config: Arc<IdpConfig>,
}

impl IdpHandlerCore {
    #[must_use]
    pub fn new(config: Arc<IdpConfig>) -> Self {
        Self { config }
    }

    pub async fn discovery(
        &self,
        _bag: &ContextBag,
        _req: &SimpleIncomingRequest,
    ) -> Result<serde_json::Value, IdpError> {
        let doc = OidcDiscoveryDocument::from_config(&self.config);
        serde_json::to_value(&doc).map_err(|e| IdpError::Internal(e.to_string()))
    }

    pub async fn jwks(
        &self,
        _bag: &ContextBag,
        _req: &SimpleIncomingRequest,
    ) -> Result<serde_json::Value, IdpError> {
        let public_pem = self
            .config
            .signing_key
            .public_key_pem()
            .map_err(|e| IdpError::Internal(e.to_string()))?;

        Ok(serde_json::json!({
            "keys": [{
                "kty": "OKP",
                "crv": "Ed25519",
                "use": "sig",
                "kid": "default",
                "alg": "EdDSA",
                "x": public_pem,
            }]
        }))
    }

    pub async fn authorize(
        &self,
        _bag: &ContextBag,
        _req: &SimpleIncomingRequest,
    ) -> Result<serde_json::Value, IdpError> {
        Err(IdpError::BadRequest(
            "Authorize endpoint requires user session and storage backend".into(),
        ))
    }

    pub async fn token(
        &self,
        _bag: &ContextBag,
        _req: &SimpleIncomingRequest,
    ) -> Result<serde_json::Value, IdpError> {
        Err(IdpError::BadRequest(
            "Token endpoint requires storage backend".into(),
        ))
    }

    pub async fn userinfo(
        &self,
        _bag: &ContextBag,
        _req: &SimpleIncomingRequest,
    ) -> Result<serde_json::Value, IdpError> {
        Err(IdpError::Unauthorized("Bearer token required".into()))
    }

    pub async fn introspect(
        &self,
        _bag: &ContextBag,
        _req: &SimpleIncomingRequest,
    ) -> Result<serde_json::Value, IdpError> {
        Ok(serde_json::json!({ "active": false }))
    }

    pub async fn device_authorize(
        &self,
        _bag: &ContextBag,
        _req: &SimpleIncomingRequest,
    ) -> Result<serde_json::Value, IdpError> {
        Err(IdpError::BadRequest(
            "Device authorize endpoint requires storage backend".into(),
        ))
    }

    pub async fn dispatch(
        &self,
        bag: &ContextBag,
        req: &SimpleIncomingRequest,
    ) -> Result<serde_json::Value, IdpError> {
        let path = req.request_url.url.as_str();
        let path = path.split('?').next().unwrap_or(path);

        if path.ends_with("/.well-known/openid-configuration") {
            self.discovery(bag, req).await
        } else if path.ends_with("/.well-known/jwks.json") {
            self.jwks(bag, req).await
        } else if path.ends_with("/authorize") && !path.ends_with("/device/authorize") {
            self.authorize(bag, req).await
        } else if path.ends_with("/token") {
            self.token(bag, req).await
        } else if path.ends_with("/userinfo") {
            self.userinfo(bag, req).await
        } else if path.ends_with("/introspect") {
            self.introspect(bag, req).await
        } else if path.ends_with("/device/authorize") {
            self.device_authorize(bag, req).await
        } else {
            Err(IdpError::BadRequest(format!("Unknown endpoint: {path}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block_on<F: core::future::Future>(f: F) -> F::Output {
        let mut f = core::pin::pin!(f);
        let waker = noop_waker();
        let mut cx = core::task::Context::from_waker(&waker);
        match f.as_mut().poll(&mut cx) {
            core::task::Poll::Ready(v) => v,
            core::task::Poll::Pending => panic!("future not ready"),
        }
    }

    fn noop_waker() -> core::task::Waker {
        use core::task::{RawWaker, RawWakerVTable};
        fn no_op(_: *const ()) {}
        fn clone(p: *const ()) -> RawWaker {
            RawWaker::new(p, &VTABLE)
        }
        const VTABLE: RawWakerVTable =
            RawWakerVTable::new(clone, no_op, no_op, no_op);
        unsafe { core::task::Waker::from_raw(RawWaker::new(core::ptr::null(), &VTABLE)) }
    }

    fn test_config() -> Arc<IdpConfig> {
        Arc::new(IdpConfig::new("https://auth.example.com".into()))
    }

    #[test]
    fn test_discovery() {
        let core = IdpHandlerCore::new(test_config());
        let bag = ContextBag::new();
        let req = SimpleIncomingRequest::builder()
            .with_plain_url("/.well-known/openid-configuration")
            .build()
            .unwrap();
        let result = block_on(core.discovery(&bag, &req));
        assert!(result.is_ok());
        let doc = result.unwrap();
        assert_eq!(doc["issuer"], "https://auth.example.com");
        assert_eq!(doc["token_endpoint"], "https://auth.example.com/token");
    }

    #[test]
    fn test_jwks() {
        let core = IdpHandlerCore::new(test_config());
        let bag = ContextBag::new();
        let req = SimpleIncomingRequest::builder()
            .with_plain_url("/.well-known/jwks.json")
            .build()
            .unwrap();
        let result = block_on(core.jwks(&bag, &req));
        assert!(result.is_ok());
        let jwks = result.unwrap();
        assert!(jwks["keys"].is_array());
        assert_eq!(jwks["keys"][0]["alg"], "EdDSA");
    }

    #[test]
    fn test_introspect_returns_inactive() {
        let core = IdpHandlerCore::new(test_config());
        let bag = ContextBag::new();
        let req = SimpleIncomingRequest::builder()
            .with_plain_url("/introspect")
            .build()
            .unwrap();
        let result = block_on(core.introspect(&bag, &req));
        assert!(result.is_ok());
        assert_eq!(result.unwrap()["active"], false);
    }

    #[test]
    fn test_discovery_document_fields() {
        let config = IdpConfig::new("https://auth.example.com".into());
        let doc = OidcDiscoveryDocument::from_config(&config);
        assert_eq!(doc.issuer, "https://auth.example.com");
        assert_eq!(doc.authorization_endpoint, "https://auth.example.com/authorize");
        assert_eq!(doc.jwks_uri, "https://auth.example.com/.well-known/jwks.json");
        assert!(doc.grant_types_supported.contains(&"authorization_code".to_string()));
        assert!(doc.code_challenge_methods_supported.contains(&"S256".to_string()));
    }
}
