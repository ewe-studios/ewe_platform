//! Proxy support for HTTP client — shared types.
//!
//! WHY: HTTP clients need proxy configuration. These types are pure data
//! with no native socket/TLS dependencies, so they can be shared with wasm32.

use crate::shared::http::HttpClientError;
use foundation_core::url::Scheme;

/// Proxy protocol type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyProtocol {
    /// HTTP proxy using CONNECT method for HTTPS targets
    Http,
    /// HTTPS proxy (TLS to proxy, then CONNECT tunnel)
    Https,
    /// SOCKS5 proxy (feature-gated, requires "socks5" feature)
    #[cfg(feature = "socks5")]
    Socks5,
}

/// Proxy authentication credentials.
#[derive(Debug, Clone)]
pub struct ProxyAuth {
    pub username: String,
    pub password: String,
}

impl ProxyAuth {
    #[must_use]
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
        }
    }

    #[must_use]
    pub fn to_basic_auth(&self) -> String {
        use base64::{engine::general_purpose, Engine as _};
        let credentials = format!("{}:{}", self.username, self.password);
        general_purpose::STANDARD.encode(credentials.as_bytes())
    }
}

/// Proxy configuration.
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub protocol: ProxyProtocol,
    pub host: String,
    pub port: u16,
    pub auth: Option<ProxyAuth>,
}

impl ProxyConfig {
    #[must_use]
    pub fn new(protocol: ProxyProtocol, host: impl Into<String>, port: u16) -> Self {
        Self {
            protocol,
            host: host.into(),
            port,
            auth: None,
        }
    }

    #[must_use]
    pub fn with_auth(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.auth = Some(ProxyAuth::new(username, password));
        self
    }

    pub fn parse(url: &str) -> Result<Self, HttpClientError> {
        let (protocol_str, rest) = url.split_once("://").ok_or_else(|| {
            HttpClientError::InvalidProxyUrl("Missing protocol separator ://".to_string())
        })?;

        let protocol = match protocol_str.to_lowercase().as_str() {
            "http" => ProxyProtocol::Http,
            "https" => ProxyProtocol::Https,
            #[cfg(feature = "socks5")]
            "socks5" => ProxyProtocol::Socks5,
            #[cfg(not(feature = "socks5"))]
            "socks5" => {
                return Err(HttpClientError::InvalidProxyUrl(
                    "SOCKS5 support requires 'socks5' feature".to_string(),
                ))
            }
            other => {
                return Err(HttpClientError::InvalidProxyUrl(format!(
                    "Unsupported proxy protocol: {other}"
                )))
            }
        };

        let (auth, host_port) = if let Some((auth_str, host_port)) = rest.rsplit_once('@') {
            let (username, password) = auth_str.split_once(':').ok_or_else(|| {
                HttpClientError::InvalidProxyUrl(
                    "Invalid auth format, expected user:pass".to_string(),
                )
            })?;
            (Some(ProxyAuth::new(username, password)), host_port)
        } else {
            (None, rest)
        };

        let (host, port_str) = host_port.rsplit_once(':').ok_or_else(|| {
            HttpClientError::InvalidProxyUrl("Missing port in proxy URL".to_string())
        })?;

        let port: u16 = port_str.parse().map_err(|_| {
            HttpClientError::InvalidProxyUrl(format!("Invalid port number: {port_str}"))
        })?;

        Ok(Self {
            protocol,
            host: host.to_string(),
            port,
            auth,
        })
    }

    #[must_use]
    pub fn from_env(scheme: &Scheme) -> Option<Self> {
        if scheme.is_http() {
            Self::from_env_var("HTTP_PROXY").or_else(|| Self::from_env_var("http_proxy"))
        } else if scheme.is_https() {
            Self::from_env_var("HTTPS_PROXY").or_else(|| Self::from_env_var("https_proxy"))
        } else {
            None
        }
    }

    #[must_use]
    pub fn should_bypass(host: &str) -> bool {
        let no_proxy = std::env::var("NO_PROXY")
            .or_else(|_| std::env::var("no_proxy"))
            .unwrap_or_default();

        if no_proxy.is_empty() {
            return false;
        }

        for pattern in no_proxy.split(',') {
            let pattern = pattern.trim();
            if pattern == "*" {
                return true;
            }
            if host == pattern {
                return true;
            }
            if pattern.starts_with('.') && host.ends_with(pattern) {
                return true;
            }
            if host.ends_with(&format!(".{pattern}")) {
                return true;
            }
        }

        false
    }

    fn from_env_var(var: &str) -> Option<Self> {
        std::env::var(var)
            .ok()
            .and_then(|url| Self::parse(&url).ok())
    }
}
