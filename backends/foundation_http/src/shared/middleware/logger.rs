//! Logger middleware — logs incoming requests with method, path, and timing.
//!
//! WHY: Request logging is essential for debugging, monitoring, and analytics.
//! WHAT: Logs request method, path, client IP, and duration at configurable levels.

use std::sync::Arc;
use std::time::Instant;

use foundation_core::wire::simple_http::SimpleIncomingRequest;

use crate::shared::client_ip::ClientIp;
use crate::shared::context::ContextBag;
use crate::shared::middleware::{MiddlewareResult, RequestMiddleware};

/// Log level for request logging.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    /// TRACE level — very verbose, includes all details.
    Trace,
    /// DEBUG level — detailed logs for development.
    Debug,
    /// INFO level — standard request logging (default).
    Info,
    /// WARN level — only warnings and errors.
    Warn,
}

impl LogLevel {
    /// Log a message at this level.
    fn log(self, message: impl std::fmt::Display) {
        match self {
            LogLevel::Trace => tracing::trace!("{}", message),
            LogLevel::Debug => tracing::debug!("{}", message),
            LogLevel::Info => tracing::info!("{}", message),
            LogLevel::Warn => tracing::warn!("{}", message),
        }
    }
}

/// Configuration for the logger middleware.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone)]
pub struct LoggerConfig {
    /// Log level for incoming requests.
    pub level: LogLevel,
    /// Include client IP address.
    pub include_client_ip: bool,
    /// Include request headers.
    pub include_headers: bool,
    /// Include query string.
    pub include_query: bool,
    /// Log response time (requires response wrapper).
    pub log_response_time: bool,
}

impl LoggerConfig {
    /// Create a new logger config with sensible defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            level: LogLevel::Info,
            include_client_ip: true,
            include_headers: false,
            include_query: true,
            log_response_time: true,
        }
    }

    /// Set the log level.
    #[must_use]
    pub fn with_level(mut self, level: LogLevel) -> Self {
        self.level = level;
        self
    }

    /// Include client IP in logs.
    #[must_use]
    pub fn with_client_ip(mut self) -> Self {
        self.include_client_ip = true;
        self
    }

    /// Include request headers in logs.
    #[must_use]
    pub fn with_headers(mut self) -> Self {
        self.include_headers = true;
        self
    }

    /// Include query string in logs.
    #[must_use]
    pub fn with_query(mut self) -> Self {
        self.include_query = true;
        self
    }

    /// Log response time.
    #[must_use]
    pub fn with_response_time(mut self) -> Self {
        self.log_response_time = true;
        self
    }
}

impl Default for LoggerConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Logger middleware.
///
/// Logs each incoming request with method, path, and optional details.
pub struct LoggerMiddleware {
    config: LoggerConfig,
}

impl LoggerMiddleware {
    /// Create a new logger middleware with the given config.
    #[must_use]
    pub fn new(config: LoggerConfig) -> Self {
        Self { config }
    }

    /// Create a logger middleware with default config.
    #[must_use]
    pub fn defaults() -> Self {
        Self::new(LoggerConfig::default())
    }

    /// Create a logger middleware with INFO level.
    #[must_use]
    pub fn info() -> Self {
        Self::new(LoggerConfig::new().with_level(LogLevel::Info))
    }

    /// Create a logger middleware with DEBUG level.
    #[must_use]
    pub fn debug() -> Self {
        Self::new(LoggerConfig::new().with_level(LogLevel::Debug))
    }

    /// Extract client IP from request extensions.
    fn extract_client_ip(req: &SimpleIncomingRequest) -> Option<String> {
        req.extensions
            .as_ref()
            .and_then(|ext| ext.get::<ClientIp>())
            .map(|cip| cip.0.clone())
    }
}

impl Default for LoggerMiddleware {
    fn default() -> Self {
        Self::new(LoggerConfig::default())
    }
}

impl RequestMiddleware for LoggerMiddleware {
    fn handle(
        &self,
        _ctx: &Arc<ContextBag>,
        req: &mut SimpleIncomingRequest,
    ) -> MiddlewareResult {
        let _start = Instant::now();

        let method = format!("{}", req.method);
        let path = &req.request_url.url;

        // Build log message
        let mut parts = Vec::new();
        parts.push(format!("{method} {path}"));

        if self.config.include_client_ip {
            if let Some(ip) = Self::extract_client_ip(req) {
                parts.push(format!("client_ip={ip}"));
            }
        }

        if self.config.include_query {
            if let Some(ref query) = req.request_url.queries {
                let query_str: Vec<String> = query
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect();
                if !query_str.is_empty() {
                    parts.push(format!("query=?{}", query_str.join("&")));
                }
            }
        }

        if self.config.include_headers {
            let headers: Vec<String> = req
                .headers
                .iter()
                .map(|(k, v)| {
                    let values = v.join(", ");
                    format!("{k}: {values}")
                })
                .collect();
            if !headers.is_empty() {
                parts.push(format!("headers=[{}]", headers.join(", ")));
            }
        }

        // Store start time in request extensions for response time logging
        // This would require extending SimpleIncomingRequest.extensions
        // For now, we just log the request start
        self.config.level.log(parts.join(" | "));

        // Note: Response time logging would require:
        // 1. Storing start time in request extensions
        // 2. A response wrapper that logs after handler completes
        // This is a simplified implementation that logs request arrival only.

        MiddlewareResult::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logger_config_default() {
        let config = LoggerConfig::default();
        assert_eq!(config.level, LogLevel::Info);
        assert!(config.include_client_ip);
        assert!(!config.include_headers);
        assert!(config.include_query);
        assert!(config.log_response_time);
    }

    #[test]
    fn test_logger_middleware_info() {
        let mw = LoggerMiddleware::info();
        assert_eq!(mw.config.level, LogLevel::Info);
    }

    #[test]
    fn test_logger_middleware_debug() {
        let mw = LoggerMiddleware::debug();
        assert_eq!(mw.config.level, LogLevel::Debug);
    }
}
