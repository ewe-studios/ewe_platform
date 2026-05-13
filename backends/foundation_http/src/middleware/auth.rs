//! Auth middleware — bearer token and basic auth validation.
//!
//! WHY: Authentication is required for protected endpoints.
//! WHAT: Validates Authorization header for Bearer tokens and Basic auth.

use std::sync::Arc;

use foundation_core::wire::simple_http::{
    SimpleHeader, SimpleIncomingRequest, SimpleOutgoingResponse, SendSafeBody, Status,
};

use crate::context::ContextBag;
use crate::middleware::{MiddlewareResult, RequestMiddleware};

/// Authentication result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthResult {
    /// Authentication successful.
    Authenticated,
    /// Missing credentials.
    Missing,
    /// Invalid credentials.
    Invalid(String),
}

/// Authentication configuration.
#[derive(Clone)]
pub struct AuthConfig {
    /// Bearer token validation function.
    pub bearer_validator: Option<Arc<dyn Fn(&str) -> bool + Send + Sync>>,
    /// Basic auth validation function (username, password) -> bool.
    pub basic_validator: Option<Arc<dyn Fn(&str, &str) -> bool + Send + Sync>>,
    /// Allowed paths (regex patterns) that bypass auth.
    pub public_paths: Vec<String>,
    /// Custom realm for WWW-Authenticate header.
    pub realm: String,
}

impl AuthConfig {
    /// Create a new auth config.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bearer_validator: None,
            basic_validator: None,
            public_paths: Vec::new(),
            realm: "Protected".to_string(),
        }
    }

    /// Set the bearer token validator.
    #[must_use]
    pub fn with_bearer_validator<F>(mut self, validator: F) -> Self
    where
        F: Fn(&str) -> bool + Send + Sync + 'static,
    {
        self.bearer_validator = Some(Arc::new(validator));
        self
    }

    /// Set the basic auth validator.
    #[must_use]
    pub fn with_basic_validator<F>(mut self, validator: F) -> Self
    where
        F: Fn(&str, &str) -> bool + Send + Sync + 'static,
    {
        self.basic_validator = Some(Arc::new(validator));
        self
    }

    /// Add a public path pattern that bypasses auth.
    #[must_use]
    pub fn with_public_path(mut self, path: impl Into<String>) -> Self {
        self.public_paths.push(path.into());
        self
    }

    /// Set the authentication realm.
    #[must_use]
    pub fn with_realm(mut self, realm: impl Into<String>) -> Self {
        self.realm = realm.into();
        self
    }

    /// Check if a path is public (bypasses auth).
    fn is_public_path(&self, path: &str) -> bool {
        self.public_paths.iter().any(|p| {
            // Simple prefix matching; could use regex for patterns
            path.starts_with(p)
        })
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Authentication middleware.
///
/// Validates Bearer tokens or Basic auth credentials.
pub struct AuthMiddleware {
    config: AuthConfig,
}

impl AuthMiddleware {
    /// Create a new auth middleware with the given config.
    #[must_use]
    pub fn new(config: AuthConfig) -> Self {
        Self { config }
    }

    /// Create an auth middleware with bearer token validation.
    #[must_use]
    pub fn bearer<F>(validator: F) -> Self
    where
        F: Fn(&str) -> bool + Send + Sync + 'static,
    {
        Self::new(AuthConfig::new().with_bearer_validator(validator))
    }

    /// Create an auth middleware with basic auth validation.
    #[must_use]
    pub fn basic<F>(validator: F) -> Self
    where
        F: Fn(&str, &str) -> bool + Send + Sync + 'static,
    {
        Self::new(AuthConfig::new().with_basic_validator(validator))
    }

    /// Extract the Authorization header value.
    fn extract_auth_header(req: &SimpleIncomingRequest) -> Option<String> {
        req.headers
            .iter()
            .find(|(k, _)| format!("{k}").eq_ignore_ascii_case("authorization"))
            .and_then(|(_, v)| v.first().cloned())
    }

    /// Validate a Bearer token.
    fn validate_bearer(&self, token: &str) -> AuthResult {
        if let Some(ref validator) = self.config.bearer_validator {
            if validator(token) {
                AuthResult::Authenticated
            } else {
                AuthResult::Invalid("Invalid bearer token".to_string())
            }
        } else {
            AuthResult::Invalid("Bearer auth not configured".to_string())
        }
    }

    /// Validate Basic auth credentials.
    fn validate_basic(&self, credentials: &str) -> AuthResult {
        // Decode base64 credentials
        let decoded = match base64::decode(credentials) {
            Ok(d) => d,
            Err(_) => return AuthResult::Invalid("Invalid base64 encoding".to_string()),
        };

        let creds = match String::from_utf8(decoded) {
            Ok(s) => s,
            Err(_) => return AuthResult::Invalid("Invalid UTF-8 in credentials".to_string()),
        };

        // Split username:password
        let parts: Vec<&str> = creds.splitn(2, ':').collect();
        if parts.len() != 2 {
            return AuthResult::Invalid("Invalid credentials format".to_string());
        }

        let username = parts[0];
        let password = parts[1];

        if let Some(ref validator) = self.config.basic_validator {
            if validator(username, password) {
                AuthResult::Authenticated
            } else {
                AuthResult::Invalid("Invalid username or password".to_string())
            }
        } else {
            AuthResult::Invalid("Basic auth not configured".to_string())
        }
    }

    /// Validate the Authorization header.
    fn validate_auth(&self, auth_header: &str) -> AuthResult {
        let auth_lower = auth_header.to_lowercase();

        if auth_lower.starts_with("bearer ") {
            let token = auth_header[7..].trim();
            self.validate_bearer(token)
        } else if auth_lower.starts_with("basic ") {
            let credentials = auth_header[6..].trim();
            self.validate_basic(credentials)
        } else {
            AuthResult::Invalid("Unsupported auth scheme".to_string())
        }
    }

    /// Build the 401 Unauthorized response.
    fn unauthorized_response(&self) -> SimpleOutgoingResponse {
        let www_auth = format!("Bearer, Basic realm=\"{}\"", self.config.realm);

        SimpleOutgoingResponse::builder()
            .with_status(Status::Unauthorized)
            .add_header(SimpleHeader::WWW_AUTHENTICATE, &www_auth)
            .with_body(SendSafeBody::Text("Unauthorized".into()))
            .build()
            .expect("valid 401 response")
    }
}

impl RequestMiddleware for AuthMiddleware {
    fn handle(
        &self,
        _ctx: &Arc<ContextBag>,
        req: &mut SimpleIncomingRequest,
    ) -> MiddlewareResult {
        let path = &req.request_url.url;

        // Check if path is public
        if self.config.is_public_path(path) {
            return MiddlewareResult::Continue;
        }

        // Extract and validate Authorization header
        match Self::extract_auth_header(req) {
            Some(auth_header) => {
                match self.validate_auth(&auth_header) {
                    AuthResult::Authenticated => MiddlewareResult::Continue,
                    AuthResult::Missing => {
                        // This shouldn't happen since we have a header, but handle it
                        MiddlewareResult::Response(self.unauthorized_response())
                    }
                    AuthResult::Invalid(reason) => {
                        tracing::warn!("Auth failed: {}", reason);
                        MiddlewareResult::Response(self.unauthorized_response())
                    }
                }
            }
            None => MiddlewareResult::Response(self.unauthorized_response()),
        }
    }
}

/// Simple base64 decoding helper (since we may not have a base64 crate).
mod base64 {
    /// Decode base64 string to bytes.
    pub fn decode(input: &str) -> Result<Vec<u8>, ()> {
        // Standard base64 alphabet
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

        // Remove padding
        let input = input.trim_end_matches('=');

        if input.is_empty() {
            return Ok(Vec::new());
        }

        // Check for invalid characters
        for c in input.chars() {
            if !c.is_ascii_alphanumeric() && c != '+' && c != '/' {
                return Err(());
            }
        }

        // Simple base64 decoder
        let mut result = Vec::with_capacity(input.len() * 3 / 4);
        let mut buffer: u32 = 0;
        let mut bits_collected: u8 = 0;

        for c in input.chars() {
            let value = if c == '+' {
                62
            } else if c == '/' {
                63
            } else if c.is_ascii_digit() {
                c as u8 - b'0' + 52
            } else if c.is_ascii_lowercase() {
                c as u8 - b'a' + 26
            } else if c.is_ascii_uppercase() {
                c as u8 - b'A'
            } else {
                continue; // Skip invalid characters
            } as u32;

            buffer = (buffer << 6) | value;
            bits_collected += 6;

            if bits_collected >= 8 {
                bits_collected -= 8;
                let byte = ((buffer >> bits_collected) & 0xFF) as u8;
                result.push(byte);
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_config_new() {
        let config = AuthConfig::new();
        assert!(config.bearer_validator.is_none());
        assert!(config.basic_validator.is_none());
        assert!(config.public_paths.is_empty());
        assert_eq!(config.realm, "Protected");
    }

    #[test]
    fn test_auth_config_with_bearer() {
        let config = AuthConfig::new().with_bearer_validator(|token| token == "valid-token");
        assert!(config.bearer_validator.is_some());
    }

    #[test]
    fn test_auth_middleware_bearer() {
        let mw = AuthMiddleware::bearer(|token| token == "valid-token");
        assert!(mw.config.bearer_validator.is_some());
    }

    #[test]
    fn test_auth_middleware_basic() {
        let mw = AuthMiddleware::basic(|user, pass| user == "admin" && pass == "secret");
        assert!(mw.config.basic_validator.is_some());
    }

    #[test]
    fn test_base64_decode_empty() {
        assert_eq!(base64::decode("").unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn test_base64_decode_simple() {
        // "dXNlcjpwYXNz" = "user:pass" in base64
        let result = base64::decode("dXNlcjpwYXNz").unwrap();
        assert_eq!(result, b"user:pass");
    }

    #[test]
    fn test_base64_decode_with_padding() {
        // "YQ==" = "a" in base64
        let result = base64::decode("YQ==").unwrap();
        assert_eq!(result, b"a");
    }
}
