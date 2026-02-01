use crate::wire::simple_http::client::errors::DnsError;
use std::collections::HashMap;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Trait for DNS resolution.
///
/// Allows pluggable DNS resolvers for testing and customization.
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
#[derive(Debug)]
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
        self.cache.lock().map(|c| c.len()).unwrap_or(0)
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
    responses: Arc<Mutex<HashMap<String, Result<Vec<SocketAddr>, DnsError>>>>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};
    use std::thread;
    use std::time::Duration;

    /// WHY: Verify SystemDnsResolver can resolve localhost
    /// WHAT: Tests basic DNS resolution functionality with a known hostname
    #[test]
    fn test_system_resolver_resolves_localhost() {
        let resolver = SystemDnsResolver::new();
        let result = resolver.resolve("localhost", 8080);

        assert!(result.is_ok(), "localhost should resolve successfully");
        let addrs = result.unwrap();
        assert!(
            !addrs.is_empty(),
            "localhost should have at least one address"
        );
    }

    /// WHY: Verify SystemDnsResolver rejects empty hostnames
    /// WHAT: Tests that invalid input is properly rejected
    #[test]
    fn test_system_resolver_rejects_empty_host() {
        let resolver = SystemDnsResolver::new();
        let result = resolver.resolve("", 8080);

        assert!(result.is_err(), "empty hostname should fail");
        matches!(result.unwrap_err(), DnsError::InvalidHost(_));
    }

    /// WHY: Verify SystemDnsResolver handles invalid hostnames
    /// WHAT: Tests error handling for hostnames that don't resolve
    #[test]
    fn test_system_resolver_handles_invalid_host() {
        let resolver = SystemDnsResolver::new();
        let result = resolver.resolve("this-hostname-should-not-exist-12345.invalid", 8080);

        assert!(result.is_err(), "invalid hostname should fail to resolve");
    }

    /// WHY: Verify MockDnsResolver returns configured responses
    /// WHAT: Tests that mock resolver can be configured with specific addresses
    #[test]
    fn test_mock_resolver_returns_configured_response() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let resolver = MockDnsResolver::new().with_response("example.com", vec![addr]);

        let result = resolver.resolve("example.com", 80);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![addr]);
    }

    /// WHY: Verify MockDnsResolver returns configured errors
    /// WHAT: Tests that mock resolver can simulate DNS failures
    #[test]
    fn test_mock_resolver_returns_configured_error() {
        let resolver = MockDnsResolver::new().with_error(
            "error.com",
            DnsError::ResolutionFailed("test error".to_string()),
        );

        let result = resolver.resolve("error.com", 80);
        assert!(result.is_err());
        matches!(result.unwrap_err(), DnsError::ResolutionFailed(_));
    }

    /// WHY: Verify MockDnsResolver returns NoAddressesFound for unconfigured hosts
    /// WHAT: Tests default behavior when no response is configured
    #[test]
    fn test_mock_resolver_returns_not_found_for_unconfigured_host() {
        let resolver = MockDnsResolver::new();
        let result = resolver.resolve("unconfigured.com", 80);

        assert!(result.is_err());
        matches!(result.unwrap_err(), DnsError::NoAddressesFound(_));
    }

    /// WHY: Verify CachingDnsResolver caches successful resolutions
    /// WHAT: Tests that resolved addresses are cached and reused
    #[test]
    fn test_caching_resolver_caches_results() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let mock = MockDnsResolver::new().with_response("cache-test.com", vec![addr]);
        let resolver = CachingDnsResolver::new(mock, Duration::from_secs(60));

        // First resolution - cache miss
        assert_eq!(resolver.cache_size(), 0);
        let result1 = resolver.resolve("cache-test.com", 80);
        assert!(result1.is_ok());
        assert_eq!(resolver.cache_size(), 1);

        // Second resolution - cache hit
        let result2 = resolver.resolve("cache-test.com", 80);
        assert!(result2.is_ok());
        assert_eq!(result1.unwrap(), result2.unwrap());
        assert_eq!(resolver.cache_size(), 1);
    }

    /// WHY: Verify CachingDnsResolver expires old entries
    /// WHAT: Tests that cached entries expire after TTL and are re-resolved
    #[test]
    fn test_caching_resolver_expires_entries() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let mock = MockDnsResolver::new().with_response("expire-test.com", vec![addr]);
        let resolver = CachingDnsResolver::new(mock, Duration::from_millis(100));

        // First resolution
        let result1 = resolver.resolve("expire-test.com", 80);
        assert!(result1.is_ok());
        assert_eq!(resolver.cache_size(), 1);

        // Wait for expiration
        thread::sleep(Duration::from_millis(150));

        // Second resolution after expiration
        let result2 = resolver.resolve("expire-test.com", 80);
        assert!(result2.is_ok());
        // Cache still has 1 entry (replaced the expired one)
        assert_eq!(resolver.cache_size(), 1);
    }

    /// WHY: Verify CachingDnsResolver clear_cache works
    /// WHAT: Tests that manual cache clearing removes all entries
    #[test]
    fn test_caching_resolver_clear_cache() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let mock = MockDnsResolver::new().with_response("clear-test.com", vec![addr]);
        let resolver = CachingDnsResolver::new(mock, Duration::from_secs(60));

        // Populate cache
        resolver.resolve("clear-test.com", 80).unwrap();
        assert_eq!(resolver.cache_size(), 1);

        // Clear cache
        resolver.clear_cache();
        assert_eq!(resolver.cache_size(), 0);
    }

    /// WHY: Verify CachingDnsResolver propagates errors from inner resolver
    /// WHAT: Tests that DNS resolution errors are not cached and properly propagated
    #[test]
    fn test_caching_resolver_propagates_errors() {
        let mock = MockDnsResolver::new()
            .with_error("error.com", DnsError::ResolutionFailed("test".to_string()));
        let resolver = CachingDnsResolver::new(mock, Duration::from_secs(60));

        let result = resolver.resolve("error.com", 80);
        assert!(result.is_err());

        // Error should not be cached
        assert_eq!(resolver.cache_size(), 0);
    }

    /// WHY: Verify CachingDnsResolver differentiates by port
    /// WHAT: Tests that same host with different ports creates separate cache entries
    #[test]
    fn test_caching_resolver_differentiates_by_port() {
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 80);
        let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 443);
        let mock = MockDnsResolver::new()
            .with_response("port-test.com", vec![addr1])
            .with_response("port-test.com", vec![addr2]);

        let resolver = CachingDnsResolver::new(mock, Duration::from_secs(60));

        // Resolve with different ports
        resolver.resolve("port-test.com", 80).unwrap();
        resolver.resolve("port-test.com", 443).unwrap();

        // Should have 2 cache entries
        assert_eq!(resolver.cache_size(), 2);
    }

    /// WHY: Verify DnsResolver trait is Send + Sync
    /// WHAT: Tests that resolvers can be used across threads
    #[test]
    fn test_dns_resolver_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SystemDnsResolver>();
        assert_send_sync::<MockDnsResolver>();
        assert_send_sync::<CachingDnsResolver<SystemDnsResolver>>();
    }
}
