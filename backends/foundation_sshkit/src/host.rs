//! Remote host reachable via SSH.

use std::path::PathBuf;

/// A remote host reachable via SSH. Parsed from `user@host:port` strings.
#[derive(Debug, Clone)]
pub struct Host {
    pub hostname: String,
    pub port: u16,
    pub user: String,
    pub password: Option<String>,
    pub key_paths: Vec<PathBuf>,
    pub proxy: Option<Box<Host>>,
    pub properties: serde_json::Value,
}

impl Host {
    /// Parse from "user@host:port" or "host".
    pub fn parse(s: &str) -> Self {
        let (user, rest) = if let Some((u, r)) = s.split_once('@') {
            (u.to_string(), r.to_string())
        } else {
            ("root".to_string(), s.to_string())
        };

        let (hostname, port) = if let Some((h, p)) = rest.split_once(':') {
            (h.to_string(), p.parse().unwrap_or(22))
        } else {
            (rest, 22)
        };

        Self {
            hostname,
            port,
            user,
            password: None,
            key_paths: Vec::new(),
            proxy: None,
            properties: serde_json::Value::Object(Default::default()),
        }
    }

    /// Set the SSH key path.
    #[must_use]
    pub fn with_key(mut self, path: impl Into<PathBuf>) -> Self {
        self.key_paths.push(path.into());
        self
    }

    /// Set the password.
    #[must_use]
    pub fn with_password(mut self, pw: impl Into<String>) -> Self {
        self.password = Some(pw.into());
        self
    }

    /// Set a bastion/jump host.
    #[must_use]
    pub fn via(mut self, proxy: Host) -> Self {
        self.proxy = Some(Box::new(proxy));
        self
    }
}

impl From<&str> for Host {
    fn from(s: &str) -> Self { Self::parse(s) }
}
