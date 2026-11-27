//! The Cross-Origin Resource Sharing (CORS) answer of a browser-facing host
//! (spec 83 feature 05, section 6.5).
//!
//! WHY: a page on one origin calls a host on another origin. The browser sends
//! a preflight `OPTIONS` request and reads the response headers before it sends
//! the call. A host that gives no CORS headers is unreachable from a browser.
//!
//! WHAT: [`CorsConfig`] (the policy) and the two header lists that a host
//! writes: [`CorsConfig::preflight_headers`] and
//! [`CorsConfig::response_headers`].
//!
//! HOW: an exact origin list, never `*` with credentials. A request from an
//! origin that is not in the list gets no CORS headers, so the browser blocks
//! the page from reading the response. The defaults are the list in the CORS
//! guide of Connect (`connectrpc.com/cors`) plus `Authorization`.

use foundation_compact::Duration;

/// The CORS policy of a host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorsConfig {
    /// Exact origins, for example `https://app.example.com`. Default empty:
    /// no CORS headers, so a browser on another origin cannot call. Never
    /// `*` together with credentials.
    pub allowed_origins: Vec<String>,
    /// The headers that a call may send. Default: the list of
    /// `connectrpc.com/cors` plus `Authorization`.
    pub allowed_headers: Vec<String>,
    /// The headers that a page may read.
    pub exposed_headers: Vec<String>,
    /// How long a browser may cache the preflight answer. Default 7200
    /// seconds: the value in the CORS guide of Connect, which is also the
    /// largest value Chromium accepts.
    pub max_age: Duration,
}

impl CorsConfig {
    /// The default allowed request headers: the Connect list plus
    /// `Authorization`.
    #[must_use]
    pub fn default_allowed_headers() -> Vec<String> {
        [
            "Content-Type",
            "Connect-Protocol-Version",
            "Connect-Timeout-Ms",
            "Grpc-Timeout",
            "X-Grpc-Web",
            "X-User-Agent",
            "Authorization",
        ]
        .into_iter()
        .map(str::to_string)
        .collect()
    }

    /// The default readable response headers.
    #[must_use]
    pub fn default_exposed_headers() -> Vec<String> {
        ["Grpc-Status", "Grpc-Message", "Grpc-Status-Details-Bin"]
            .into_iter()
            .map(str::to_string)
            .collect()
    }

    /// Allow one origin.
    #[must_use]
    pub fn with_allowed_origin(mut self, origin: impl Into<String>) -> Self {
        self.allowed_origins.push(origin.into());
        self
    }

    /// Whether `origin` may call this host.
    #[must_use]
    pub fn allows(&self, origin: &str) -> bool {
        self.allowed_origins.iter().any(|allowed| allowed == origin)
    }

    /// The headers of a preflight `OPTIONS` answer for `origin`.
    ///
    /// It is empty when the origin is not allowed. Otherwise it allows `POST`
    /// and `GET` (Connect uses `GET` for idempotent unary calls), the allowed
    /// request headers and the cached lifetime.
    #[must_use]
    pub fn preflight_headers(&self, origin: &str) -> Vec<(String, String)> {
        if !self.allows(origin) {
            return Vec::new();
        }
        let max_age = self.max_age.as_secs();
        vec![
            (
                "Access-Control-Allow-Origin".to_string(),
                origin.to_string(),
            ),
            ("Vary".to_string(), "Origin".to_string()),
            (
                "Access-Control-Allow-Methods".to_string(),
                "POST, GET".to_string(),
            ),
            (
                "Access-Control-Allow-Headers".to_string(),
                self.allowed_headers.join(", "),
            ),
            ("Access-Control-Max-Age".to_string(), max_age.to_string()),
        ]
    }

    /// The headers that every response to `origin` carries.
    ///
    /// It is empty when the origin is not allowed. `Access-Control-Allow-Origin`
    /// names the exact origin, and `Vary: Origin` tells a cache that the answer
    /// depends on the origin.
    #[must_use]
    pub fn response_headers(&self, origin: &str) -> Vec<(String, String)> {
        if !self.allows(origin) {
            return Vec::new();
        }
        let mut headers = vec![
            (
                "Access-Control-Allow-Origin".to_string(),
                origin.to_string(),
            ),
            ("Vary".to_string(), "Origin".to_string()),
        ];
        if !self.exposed_headers.is_empty() {
            headers.push((
                "Access-Control-Expose-Headers".to_string(),
                self.exposed_headers.join(", "),
            ));
        }
        headers.push((
            "Access-Control-Allow-Credentials".to_string(),
            "true".to_string(),
        ));
        headers
    }
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: Vec::new(),
            allowed_headers: Self::default_allowed_headers(),
            exposed_headers: Self::default_exposed_headers(),
            max_age: Duration::from_secs(7200),
        }
    }
}
