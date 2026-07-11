use crate::simple_http::shared::DnsError;
use std::collections::HashMap;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Type alias for mock DNS response storage
type MockDnsResponses = Arc<Mutex<HashMap<String, Result<Vec<SocketAddr>, DnsError>>>>;

/// Trait for DNS resolution.
///
/// WHY: pluggable DNS resolvers for testing and customization. Kept **object-safe**
/// (no `Clone` supertrait, no generic methods) so it can be erased to
/// [`BoxedDnsResolver`] — a `Arc<dyn DnsResolver>` the client holds instead of a
/// resolver type parameter. Cloning is provided by the `Arc`, not the resolver.
///
/// WHAT: a single `resolve(host, port)` method.
///
/// HOW: implementors are concrete resolvers ([`SystemDnsResolver`],
/// [`StaticSocketAddr`], [`CachingDnsResolver`], [`MockDnsResolver`],
/// [`NoopDnsResolver`]); the client erases whichever it is built with into an
/// `Arc<dyn DnsResolver>`.
pub trait DnsResolver: Send + Sync {
    /// Resolves a hostname and port to socket addresses.
    ///
    /// # Arguments
    ///
    /// * `host` - The hostname to resolve
    /// * `port` - The port number
    ///
    /// # Returns
    ///
    /// A vector of resolved socket addresses.
    ///
    /// # Errors
    ///
    /// Returns `DnsError` if resolution fails.
    fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, DnsError>;
}

/// A DNS resolver erased to a shared trait object.
///
/// WHY: lets the client hold one concrete resolver type (`R = BoxedDnsResolver`)
/// instead of a type parameter, so `dyn HttpClient` stays object-safe. `Arc`
/// provides the cloning the (now `Clone`-free) trait no longer requires.
pub type BoxedDnsResolver = Arc<dyn DnsResolver>;

impl<T: DnsResolver + ?Sized> DnsResolver for Arc<T> {
    fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, DnsError> {
        self.as_ref().resolve(host, port)
    }
}

/// The default resolver for the current target.
///
/// Native resolves through the system ([`SystemDnsResolver`]); wasm delegates to
/// the platform (the browser resolves for `fetch`/`WebSocket`), so it defaults to
/// [`NoopDnsResolver`]. This keeps the client's default surface identical on both
/// targets — only the alias target differs.
#[cfg(not(target_family = "wasm"))]
pub type DefaultResolver = SystemDnsResolver;
/// The default resolver for the current target (wasm — the browser resolves).
#[cfg(target_family = "wasm")]
pub type DefaultResolver = NoopDnsResolver;

/// A resolver that resolves nothing — the platform owns resolution.
///
/// WHY: on wasm there is no system resolver; the browser resolves hostnames when
/// it performs the `fetch`/`WebSocket`, so this resolver's `resolve` is never on
/// the hot path. It exists so the generic client surface has a `Default` resolver
/// on wasm identical in shape to native.
///
/// WHAT: `resolve` returns an error (never a silent localhost/empty default —
/// calling it means something bypassed the platform path).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopDnsResolver;

impl DnsResolver for NoopDnsResolver {
    fn resolve(&self, host: &str, _port: u16) -> Result<Vec<SocketAddr>, DnsError> {
        Err(DnsError::ResolutionFailed(format!(
            "NoopDnsResolver cannot resolve '{host}': resolution is delegated to the \
             platform (browser fetch/WebSocket). Configure a real resolver to resolve here."
        )))
    }
}

/// `StaticSocketAddrResolver` returns a static `std::net::ToSocketAddrs`.
///
/// Useful for testing scenarios where a specific IP address is required.
#[derive(Debug, Clone)]
pub struct StaticSocketAddr(std::net::SocketAddr);

impl Default for StaticSocketAddr {
    /// Creates a new `StaticSocketAddr` with default values.
    /// It returns the localhost address as default.
    fn default() -> Self {
        Self(std::net::SocketAddr::from(([127, 0, 0, 1], 80)))
    }
}

impl StaticSocketAddr {
    /// Creates a new system DNS resolver.
    #[must_use]
    pub fn new(addr: SocketAddr) -> Self {
        Self(addr)
    }
}

impl DnsResolver for StaticSocketAddr {
    fn resolve(&self, _host: &str, _port: u16) -> Result<Vec<SocketAddr>, DnsError> {
        Ok(vec![self.0])
    }
}

/// System DNS resolver using `std::net::ToSocketAddrs`.
///
/// This is the default resolver that uses the system's DNS resolver.
#[derive(Debug, Clone, Default)]
pub struct SystemDnsResolver;

impl SystemDnsResolver {
    /// Creates a new system DNS resolver.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl DnsResolver for SystemDnsResolver {
    fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, DnsError> {
        if host.is_empty() {
            return Err(DnsError::InvalidHost(host.to_string()));
        }

        let addr_str = format!("{host}:{port}");
        let addrs: Vec<SocketAddr> = addr_str
            .to_socket_addrs()
            .map_err(DnsError::from)?
            .collect();

        if addrs.is_empty() {
            return Err(DnsError::NoAddressesFound(host.to_string()));
        }

        Ok(addrs)
    }
}

/// Cached DNS entry.
#[derive(Debug, Clone)]
struct CachedEntry {
    addresses: Vec<SocketAddr>,
    expires_at: Instant,
}

impl CachedEntry {
    /// Checks if this entry has expired.
    fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }
}

/// Caching DNS resolver that wraps another resolver.
///
/// Caches DNS resolution results with a configurable TTL.
///
/// # Type Parameters
///
/// * `R` - The inner resolver type
#[derive(Debug, Clone)]
pub struct CachingDnsResolver<R: DnsResolver> {
    inner: R,
    cache: Arc<Mutex<HashMap<String, CachedEntry>>>,
    ttl: Duration,
}

impl<R: DnsResolver> CachingDnsResolver<R> {
    /// Creates a new caching DNS resolver with the given TTL.
    ///
    /// # Arguments
    ///
    /// * `inner` - The resolver to wrap
    /// * `ttl` - Time-to-live for cache entries
    pub fn new(inner: R, ttl: Duration) -> Self {
        Self {
            inner,
            cache: Arc::new(Mutex::new(HashMap::new())),
            ttl,
        }
    }

    /// Creates a new caching resolver with default 5-minute TTL.
    pub fn with_default_ttl(inner: R) -> Self {
        Self::new(inner, Duration::from_secs(300))
    }

    /// Clears all cached entries.
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
    }

    /// Gets the number of entries in the cache.
    pub fn cache_size(&self) -> usize {
        self.cache.lock().map_or(0, |c| c.len())
    }
}

impl<R: DnsResolver> DnsResolver for CachingDnsResolver<R> {
    fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, DnsError> {
        let cache_key = format!("{host}:{port}");

        // Check cache first
        if let Ok(cache) = self.cache.lock() {
            if let Some(entry) = cache.get(&cache_key) {
                if !entry.is_expired() {
                    return Ok(entry.addresses.clone());
                }
            }
        }

        // Cache miss or expired - resolve using inner resolver
        let addresses = self.inner.resolve(host, port)?;

        // Store in cache
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(
                cache_key,
                CachedEntry {
                    addresses: addresses.clone(),
                    expires_at: Instant::now() + self.ttl,
                },
            );
        }

        Ok(addresses)
    }
}

/// Mock DNS resolver for testing.
///
/// Allows configuring responses for specific hostnames.
#[derive(Debug, Clone)]
pub struct MockDnsResolver {
    responses: MockDnsResponses,
}

impl MockDnsResolver {
    /// Creates a new mock DNS resolver.
    #[must_use]
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Configures a successful response for a hostname.
    #[must_use]
    pub fn with_response(self, host: &str, addrs: Vec<SocketAddr>) -> Self {
        if let Ok(mut responses) = self.responses.lock() {
            responses.insert(host.to_string(), Ok(addrs));
        }
        self
    }

    /// Configures an error response for a hostname.
    #[must_use]
    pub fn with_error(self, host: &str, error: DnsError) -> Self {
        if let Ok(mut responses) = self.responses.lock() {
            responses.insert(host.to_string(), Err(error));
        }
        self
    }
}

impl Default for MockDnsResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl DnsResolver for MockDnsResolver {
    fn resolve(&self, host: &str, _port: u16) -> Result<Vec<SocketAddr>, DnsError> {
        let responses = self
            .responses
            .lock()
            .map_err(|_| DnsError::ResolutionFailed("lock poisoned".to_string()))?;

        responses
            .get(host)
            .cloned()
            .unwrap_or_else(|| Err(DnsError::NoAddressesFound(host.to_string())))
    }
}
