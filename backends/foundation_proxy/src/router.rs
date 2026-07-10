//! Host + path router.
//!
//! WHY: One proxy fronts many services. A request is dispatched by its `Host`
//! header first, then by the longest matching `path_prefix`. Wildcard hosts
//! (`*.example.com`) let a whole subdomain family share one service. An unknown
//! host is a `404`, never a panic.
//!
//! WHAT: [`Router`] over a set of [`ServiceRuntime`]s, with [`Router::route`]
//! returning the best-matching service (or `None` → the handler answers `404`).
//!
//! HOW: For each request the router filters services whose host pattern matches,
//! then ranks them by `(host_specificity, prefix_len)` — an exact host beats a
//! wildcard, and among equal hosts the longest `path_prefix` wins.

use std::sync::Arc;

use crate::runtime::ServiceRuntime;

/// Routes requests to services by host and path prefix.
#[derive(Debug)]
pub struct Router {
    services: Vec<Arc<ServiceRuntime>>,
}

impl Router {
    /// Build a router over `services`.
    #[must_use]
    pub fn new(services: Vec<Arc<ServiceRuntime>>) -> Self {
        Self { services }
    }

    #[must_use]
    pub fn services(&self) -> &[Arc<ServiceRuntime>] {
        &self.services
    }

    /// Find the service that should handle a request for `host` + `path`.
    ///
    /// `host` may include a port (`app.example.com:8080`) — it is stripped before
    /// matching. Returns `None` when no service's host pattern matches or when a
    /// host matches but no service's `path_prefix` is a prefix of `path`.
    #[must_use]
    pub fn route(&self, host: &str, path: &str) -> Option<Arc<ServiceRuntime>> {
        let host = normalize_host(host);
        let path = path_only(path);

        let mut best: Option<(&Arc<ServiceRuntime>, u32, usize)> = None;
        for service in &self.services {
            let cfg = service.config();
            let Some(specificity) = host_specificity(&cfg.host, host) else {
                continue;
            };
            let prefix = cfg.path_prefix.as_deref().unwrap_or("");
            if !path_matches_prefix(path, prefix) {
                continue;
            }
            let score = (specificity, prefix.len());
            let better = match best {
                None => true,
                Some((_, best_spec, best_len)) => score > (best_spec, best_len),
            };
            if better {
                best = Some((service, specificity, prefix.len()));
            }
        }

        best.map(|(service, _, _)| Arc::clone(service))
    }
}

/// Strip the port and lowercase a `Host` header value for matching.
fn normalize_host(host: &str) -> &str {
    host.split(':').next().unwrap_or(host).trim()
}

/// Strip the query string, leaving just the path for prefix matching.
fn path_only(path: &str) -> &str {
    path.split(['?', '#']).next().unwrap_or(path)
}

/// Score how specifically `pattern` matches `host`, or `None` if it does not.
///
/// Exact (case-insensitive) match scores `2`; a wildcard `*.example.com` match
/// scores `1`, so exact hosts always win ties. A wildcard matches any non-empty
/// subdomain of its suffix but not the bare apex.
fn host_specificity(pattern: &str, host: &str) -> Option<u32> {
    if pattern.eq_ignore_ascii_case(host) {
        return Some(2);
    }
    if let Some(suffix) = pattern.strip_prefix("*.") {
        // `*.example.com` → suffix `example.com`. Match `a.example.com` (and
        // deeper) but not `example.com` itself.
        let dotted = format!(".{suffix}");
        if host.len() > dotted.len() && host.to_ascii_lowercase().ends_with(&dotted.to_ascii_lowercase())
        {
            return Some(1);
        }
    }
    None
}

/// Whether `path` falls under `prefix` on a path-segment boundary.
///
/// An empty prefix matches everything. A non-empty prefix matches when `path`
/// equals it or continues with `/` (so `/api` matches `/api` and `/api/x` but
/// not `/apix`).
fn path_matches_prefix(path: &str, prefix: &str) -> bool {
    if prefix.is_empty() || prefix == "/" {
        return true;
    }
    let prefix = prefix.strip_suffix('/').unwrap_or(prefix);
    if !path.starts_with(prefix) {
        return false;
    }
    match path.as_bytes().get(prefix.len()) {
        None => true,
        Some(&b) => b == b'/',
    }
}
