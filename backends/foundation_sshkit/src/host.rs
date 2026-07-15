//! Remote host reachable via SSH.

use std::path::PathBuf;

use ssh2_config::{ParseRule, SshConfig};

/// The OS login used when a spec carries no `user@` and `~/.ssh/config` names no
/// `User`. Mirrors OpenSSH, which defaults to the current local user (never
/// `root`). Falls back to `root` only when the environment names no user.
fn default_user() -> String {
    std::env::var("USER")
        .ok()
        .or_else(|| std::env::var("LOGNAME").ok())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "root".to_string())
}

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
    /// Parse from `user@host:port` or `host`, applying OpenSSH-style defaults
    /// (current OS user, port 22). Purely syntactic — no `~/.ssh/config` lookup;
    /// use [`Host::resolve`] for that.
    pub fn parse(s: &str) -> Self {
        let (user, rest) = match s.split_once('@') {
            Some((u, r)) => (u.to_string(), r.to_string()),
            None => (default_user(), s.to_string()),
        };

        let (hostname, port) = match rest.split_once(':') {
            Some((h, p)) => (h.to_string(), p.parse().unwrap_or(22)),
            None => (rest, 22),
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

    /// Resolve `user@alias:port` through `~/.ssh/config`, the way the OpenSSH and
    /// Docker CLIs do.
    ///
    /// WHY: `parse` is syntactic — `ssh://myserver` would try to connect to a
    /// literal host `myserver` as the current user on port 22. Real usage relies
    /// on `~/.ssh/config` to map an alias to its `HostName`, `User`, `Port`, and
    /// `IdentityFile`. This layers those in.
    ///
    /// WHAT: The alias is looked up in `~/.ssh/config`; an explicit `user@` or
    /// `:port` in the spec overrides the config, which overrides the OS defaults.
    /// `IdentityFile` entries become [`Host::key_paths`]. A missing or unparseable
    /// config is ignored (falls back to [`parse`](Self::parse) semantics).
    ///
    /// NOTE: `ProxyJump` / bastions are not resolved from config here — set one
    /// explicitly with [`Host::via`].
    #[must_use]
    pub fn resolve(spec: &str) -> Self {
        // Split the spec into explicit (optional) parts so config can fill gaps.
        let (explicit_user, rest) = match spec.split_once('@') {
            Some((u, r)) => (Some(u.to_string()), r.to_string()),
            None => (None, spec.to_string()),
        };
        let (alias, explicit_port) = match rest.split_once(':') {
            Some((h, p)) => (h.to_string(), p.parse::<u16>().ok()),
            None => (rest, None),
        };

        // Best-effort `~/.ssh/config` lookup. Missing file / parse error → None.
        let params = SshConfig::parse_default_file(ParseRule::ALLOW_UNKNOWN_FIELDS)
            .ok()
            .map(|cfg| cfg.query(&alias));

        let hostname = params
            .as_ref()
            .and_then(|p| p.host_name.clone())
            .unwrap_or(alias);
        let user = explicit_user
            .or_else(|| params.as_ref().and_then(|p| p.user.clone()))
            .unwrap_or_else(default_user);
        let port = explicit_port
            .or_else(|| params.as_ref().and_then(|p| p.port))
            .unwrap_or(22);
        let key_paths = params.and_then(|p| p.identity_file).unwrap_or_default();

        Self {
            hostname,
            port,
            user,
            password: None,
            key_paths,
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
    fn from(s: &str) -> Self {
        Self::parse(s)
    }
}
