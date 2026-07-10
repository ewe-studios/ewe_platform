//! ProxyServer — running proxy instance.

use crate::config::{ProxyConfig, ProxyError};
use tracing::info;

/// A running proxy server instance.
pub struct ProxyServer {
    config: ProxyConfig,
}

impl ProxyServer {
    /// Start the proxy from a validated configuration.
    pub async fn start(config: ProxyConfig) -> Result<Self, ProxyError> {
        info!(domain = %config.domain, services = config.services.len(), "starting proxy");

        // Validate: no duplicate hosts
        let mut hosts = std::collections::HashSet::new();
        for svc in &config.services {
            if !hosts.insert(&svc.host) {
                return Err(ProxyError::Config(format!("duplicate host: {}", svc.host)));
            }
        }

        // TODO: Cloudflare DNS bootstrap (when cloudflare feature is enabled)
        // TODO: TLS cert provisioning (LetsEncrypt ACME or Cloudflare)
        // TODO: Start the HTTP/S router + health probes

        Ok(Self { config })
    }

    #[must_use]
    pub fn config(&self) -> &ProxyConfig { &self.config }
}
