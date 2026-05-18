//! CORS middleware — handles Cross-Origin Resource Sharing headers and preflight requests.
//!
//! WHY: Browsers require CORS headers for cross-origin requests. This middleware
//! adds the necessary headers and handles OPTIONS preflight requests automatically.
//!
//! WHAT: Configurable CORS middleware with origin, method, header, and credential support.

use std::sync::Arc;

use foundation_core::wire::simple_http::{
    SimpleHeader, SimpleIncomingRequest, SimpleOutgoingResponse, SimpleMethod,
    SendSafeBody, Status,
};

use crate::shared::context::ContextBag;
use crate::shared::middleware::{MiddlewareResult, RequestMiddleware};

/// CORS middleware configuration.
///
/// Controls which origins, methods, headers, and credentials are allowed.
#[derive(Clone)]
pub struct CorsConfig {
    /// Allowed origins. Use "*" for any origin, or specific origins like "<https://example.com>".
    /// Empty means no CORS (no Access-Control-Allow-Origin header).
    pub allowed_origins: Vec<String>,
    /// Allowed HTTP methods for preflight.
    pub allowed_methods: Vec<String>,
    /// Allowed request headers for preflight.
    pub allowed_headers: Vec<String>,
    /// Headers exposed to the client in the response.
    pub exposed_headers: Vec<String>,
    /// Max age for preflight cache in seconds.
    pub max_age: u32,
    /// Allow credentials (cookies, authorization headers).
    pub allow_credentials: bool,
}

impl CorsConfig {
    /// Create permissive CORS config (allow all origins, common methods).
    #[must_use]
    pub fn permissive() -> Self {
        Self {
            allowed_origins: vec!["*".to_string()],
            allowed_methods: vec!["GET", "POST", "PUT", "DELETE", "PATCH", "OPTIONS", "HEAD"]
                .into_iter()
                .map(String::from)
                .collect(),
            allowed_headers: vec!["Content-Type", "Authorization", "X-Request-Id"]
                .into_iter()
                .map(String::from)
                .collect(),
            exposed_headers: vec!["X-Request-Id".to_string()],
            max_age: 86400, // 24 hours
            allow_credentials: false, // Cannot be true with wildcard origin
        }
    }

    /// Create restrictive CORS config (no CORS headers by default).
    #[must_use]
    pub fn restrictive() -> Self {
        Self {
            allowed_origins: Vec::new(),
            allowed_methods: Vec::new(),
            allowed_headers: Vec::new(),
            exposed_headers: Vec::new(),
            max_age: 0,
            allow_credentials: false,
        }
    }

    /// Add an allowed origin.
    #[must_use]
    pub fn with_origin(mut self, origin: impl Into<String>) -> Self {
        self.allowed_origins.push(origin.into());
        self
    }

    /// Add an allowed method.
    #[must_use]
    pub fn with_method(mut self, method: impl Into<String>) -> Self {
        self.allowed_methods.push(method.into());
        self
    }

    /// Add an allowed header.
    #[must_use]
    pub fn with_header(mut self, header: impl Into<String>) -> Self {
        self.allowed_headers.push(header.into());
        self
    }

    /// Add an exposed header.
    #[must_use]
    pub fn with_exposed_header(mut self, header: impl Into<String>) -> Self {
        self.exposed_headers.push(header.into());
        self
    }

    /// Set max age for preflight cache.
    #[must_use]
    pub fn with_max_age(mut self, seconds: u32) -> Self {
        self.max_age = seconds;
        self
    }

    /// Enable credentials.
    #[must_use]
    pub fn with_credentials(mut self) -> Self {
        self.allow_credentials = true;
        self
    }

    /// Check if the origin is allowed.
    fn is_origin_allowed(&self, origin: &str) -> bool {
        self.allowed_origins.iter().any(|o| {
            o == "*" || o.eq_ignore_ascii_case(origin)
        })
    }
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self::restrictive()
    }
}

/// CORS middleware.
///
/// Adds CORS headers to responses and handles OPTIONS preflight requests.
pub struct CorsMiddleware {
    config: CorsConfig,
}

impl CorsMiddleware {
    /// Create a new CORS middleware with the given configuration.
    #[must_use]
    pub fn new(config: CorsConfig) -> Self {
        Self { config }
    }

    /// Create a permissive CORS middleware (allow all origins).
    #[must_use]
    pub fn permissive() -> Self {
        Self::new(CorsConfig::permissive())
    }

    /// Create a restrictive CORS middleware (no CORS by default).
    #[must_use]
    pub fn restrictive() -> Self {
        Self::new(CorsConfig::restrictive())
    }

    /// Get a mutable reference to the config for customization.
    pub fn config_mut(&mut self) -> &mut CorsConfig {
        &mut self.config
    }

    /// Extract the Origin header value from the request.
    fn extract_origin(req: &SimpleIncomingRequest) -> Option<String> {
        req.headers.iter()
            .find(|(k, _)| format!("{k}").eq_ignore_ascii_case("origin"))
            .and_then(|(_, v)| v.first().cloned())
    }

    /// Build CORS headers for a response.
    fn build_cors_headers(&self, origin: Option<&str>) -> Vec<(SimpleHeader, String)> {
        let mut headers = Vec::new();

        // Access-Control-Allow-Origin
        if let Some(origin) = origin {
            if self.config.is_origin_allowed(origin) {
                headers.push((SimpleHeader::custom("Access-Control-Allow-Origin"), origin.to_string()));
            }
        } else if self.config.allowed_origins.iter().any(|o| o == "*") {
            headers.push((SimpleHeader::custom("Access-Control-Allow-Origin"), "*".to_string()));
        }

        // Access-Control-Allow-Methods
        if !self.config.allowed_methods.is_empty() {
            let methods = self.config.allowed_methods.join(", ");
            headers.push((SimpleHeader::custom("Access-Control-Allow-Methods"), methods));
        }

        // Access-Control-Allow-Headers
        if !self.config.allowed_headers.is_empty() {
            let headers_list = self.config.allowed_headers.join(", ");
            headers.push((SimpleHeader::custom("Access-Control-Allow-Headers"), headers_list));
        }

        // Access-Control-Expose-Headers
        if !self.config.exposed_headers.is_empty() {
            let exposed = self.config.exposed_headers.join(", ");
            headers.push((SimpleHeader::custom("Access-Control-Expose-Headers"), exposed));
        }

        // Access-Control-Max-Age
        if self.config.max_age > 0 {
            headers.push((SimpleHeader::custom("Access-Control-Max-Age"), self.config.max_age.to_string()));
        }

        // Access-Control-Allow-Credentials
        if self.config.allow_credentials {
            headers.push((SimpleHeader::custom("Access-Control-Allow-Credentials"), "true".to_string()));
        }

        headers
    }
}

impl RequestMiddleware for CorsMiddleware {
    fn handle(
        &self,
        _ctx: &Arc<ContextBag>,
        req: &mut SimpleIncomingRequest,
    ) -> MiddlewareResult {
        // Handle OPTIONS preflight requests
        if matches!(req.method, SimpleMethod::OPTIONS) {
            let origin = Self::extract_origin(req);

            // Check if origin is allowed
            if let Some(ref origin_str) = origin {
                if !self.config.is_origin_allowed(origin_str) {
                    // Origin not allowed — return 403
                    let response = SimpleOutgoingResponse::builder()
                        .with_status(Status::Forbidden)
                        .with_body(SendSafeBody::Text("CORS: Origin not allowed".into()))
                        .build()
                        .expect("valid 403 response");
                    return MiddlewareResult::Response(response);
                }
            }

            // Build preflight response
            let mut builder = SimpleOutgoingResponse::builder()
                .with_status(Status::OK)
                .with_body(SendSafeBody::None);

            // Add CORS headers
            for (header, value) in self.build_cors_headers(origin.as_deref()) {
                builder = builder.add_header(header, value);
            }

            let response = builder.build().expect("valid preflight response");
            return MiddlewareResult::Response(response);
        }

        // For non-OPTIONS requests, just continue — CORS headers will be added
        // to the actual response by the worker loop (which would need modification
        // to support response header injection)
        //
        // NOTE: A full implementation would wrap the response writing to inject
        // CORS headers. For now, we handle preflight and let the handler add headers.
        MiddlewareResult::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cors_config_permissive() {
        let config = CorsConfig::permissive();
        assert!(config.is_origin_allowed("https://example.com"));
        assert!(config.is_origin_allowed("*"));
    }

    #[test]
    fn test_cors_config_restrictive() {
        let config = CorsConfig::restrictive();
        assert!(!config.is_origin_allowed("https://example.com"));
    }

    #[test]
    fn test_cors_config_with_origin() {
        let config = CorsConfig::restrictive()
            .with_origin("https://example.com");
        assert!(config.is_origin_allowed("https://example.com"));
        assert!(!config.is_origin_allowed("https://other.com"));
    }
}
