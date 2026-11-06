use crate::shared::http::DnsError;
#[cfg(not(target_family = "wasm"))]
use foundation_core::valtron::Pipe;
use std::collections::HashMap;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use foundation_compact::Instant;

/// Type alias for mock DNS response storage
type MockDnsResponses = Arc<Mutex<HashMap<String, Result<Vec<SocketAddr>, DnsError>>>>;

/// Re-export the object-safety bridge so downstream resolvers (and integration
/// tests) can implement [`DnsResolver`] with the same technique the crate uses.
pub use async_trait::async_trait;

/// Trait for DNS resolution.
///
/// WHY: pluggable DNS resolvers for testing and customization. Kept **object-safe**
/// (no `Clone` supertrait, no generic methods) so it can be erased to
/// [`BoxedDnsResolver`] — a `Arc<dyn DnsResolver>` the client holds instead of a
/// resolver type parameter. Cloning is provided by the `Arc`, not the resolver.
///
/// WHAT: a single `resolve(host, port)` method. It is async so a slow lookup
/// parks its task instead of holding a valtron worker (feature 36).
///
/// HOW: the method is async via `#[async_trait]`, which returns a boxed future
/// while keeping the trait object-safe. Implementors are concrete resolvers
/// ([`SystemDnsResolver`], [`StaticSocketAddr`], [`CachingDnsResolver`],
/// [`MockDnsResolver`], [`NoopDnsResolver`]); the client erases whichever it is
/// built with into an `Arc<dyn DnsResolver>`.
#[async_trait::async_trait]
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
    /// A future that yields a vector of resolved socket addresses.
    ///
    /// # Errors
    ///
    /// Returns `DnsError` if resolution fails.
    async fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, DnsError>;

    /// Resolve synchronously on the current thread (a blocking boundary).
    ///
    /// WHY: the blocking client constructors and sync tests need an address now
    /// and run on a plain thread outside the valtron pool. This must not touch
    /// the valtron background registry, so a resolver whose async path offloads
    /// (for example [`SystemDnsResolver`]) overrides this to run the lookup
    /// inline.
    ///
    /// HOW: the default polls [`DnsResolver::resolve`] with a
    /// [`std::thread::Thread::unpark`] waker and `park`s on `Pending`. That is
    /// correct only for resolvers whose async path never submits a background
    /// job; those resolvers override it.
    ///
    /// # Errors
    ///
    /// Returns the resolver's [`DnsError`] when the lookup fails.
    fn resolve_blocking(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, DnsError> {
        struct ThreadUnpark(std::thread::Thread);

        impl std::task::Wake for ThreadUnpark {
            fn wake(self: Arc<Self>) {
                self.0.unpark();
            }

            fn wake_by_ref(self: &Arc<Self>) {
                self.0.unpark();
            }
        }

        let waker = Waker::from(Arc::new(ThreadUnpark(std::thread::current())));
        let mut cx = Context::from_waker(&waker);
        let mut fut = self.resolve(host, port);

        loop {
            match fut.as_mut().poll(&mut cx) {
                Poll::Ready(result) => return result,
                Poll::Pending => std::thread::park(),
            }
        }
    }
}

/// A DNS resolver erased to a shared trait object.
///
/// WHY: lets the client hold one concrete resolver type (`R = BoxedDnsResolver`)
/// instead of a type parameter, so `dyn HttpClient` stays object-safe. `Arc`
/// provides the cloning the (now `Clone`-free) trait no longer requires.
pub type BoxedDnsResolver = Arc<dyn DnsResolver>;

#[async_trait::async_trait]
impl<T: DnsResolver + ?Sized> DnsResolver for Arc<T> {
    async fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, DnsError> {
        self.as_ref().resolve(host, port).await
    }

    fn resolve_blocking(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, DnsError> {
        self.as_ref().resolve_blocking(host, port)
    }
}

/// Resolve synchronously on the current thread (a blocking boundary).
///
/// WHY: the resolver seam is async, but sync call sites (the blocking client
/// constructors, integration tests) still need a resolved address now. This is a
/// thin wrapper over [`DnsResolver::resolve_blocking`] so call sites do not need
/// the trait in scope.
///
/// # Errors
///
/// Returns the resolver's [`DnsError`] when the lookup fails.
pub fn resolve_blocking<R: DnsResolver + ?Sized>(
    resolver: &R,
    host: &str,
    port: u16,
) -> Result<Vec<SocketAddr>, DnsError> {
    resolver.resolve_blocking(host, port)
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

#[async_trait::async_trait]
impl DnsResolver for NoopDnsResolver {
    async fn resolve(&self, host: &str, _port: u16) -> Result<Vec<SocketAddr>, DnsError> {
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

#[async_trait::async_trait]
impl DnsResolver for StaticSocketAddr {
    async fn resolve(&self, _host: &str, _port: u16) -> Result<Vec<SocketAddr>, DnsError> {
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

    /// Blocking lookup: `to_socket_addrs()` on the current thread.
    ///
    /// WHY: standard Rust has no async `getaddrinfo`. This is the only place
    /// that calls it. Native offloads it to a blocking boundary; wasm runs it
    /// inline because the platform never routes here (the wasm `DefaultResolver`
    /// is [`NoopDnsResolver`]).
    fn lookup_blocking(host: &str, port: u16) -> Result<Vec<SocketAddr>, DnsError> {
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

    /// Offload the blocking lookup to valtron's background boundary (feature 36).
    ///
    /// WHY: `to_socket_addrs()` blocks; running it on a valtron worker would
    /// hold that worker for the whole lookup. The closure runs on the background
    /// job registry and hands the result back through a [`Pipe`].
    ///
    /// HOW: submit the closure, then await the pipe. A cancelled awaiter drops
    /// the receiver; the worker's best-effort send then reports `Closed` and is
    /// ignored.
    #[cfg(not(target_family = "wasm"))]
    async fn offload_lookup(host: String, port: u16) -> Result<Vec<SocketAddr>, DnsError> {
        use foundation_core::valtron::run_background_job;

        let (tx, rx) = Pipe::new();
        let job_host = host.clone();
        // The submit error is boxed and non-`Send`; stringify it now so the
        // future stays `Send` across the pipe await below.
        let submitted = run_background_job(move || {
            let result = Self::lookup_blocking(&job_host, port);
            let _ = tx.try_send(result);
        })
        .map_err(|err| err.to_string());

        if let Err(msg) = submitted {
            return Err(DnsError::ResolutionFailed(format!(
                "failed to submit DNS lookup to the blocking boundary: {msg}"
            )));
        }

        match rx.receive().await {
            Some(result) => result,
            None => Err(DnsError::ResolutionFailed(
                "DNS lookup worker dropped the result channel before replying".to_string(),
            )),
        }
    }
}

#[async_trait::async_trait]
impl DnsResolver for SystemDnsResolver {
    async fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, DnsError> {
        if host.is_empty() {
            return Err(DnsError::InvalidHost(host.to_string()));
        }

        let host = host.to_owned();

        #[cfg(not(target_family = "wasm"))]
        {
            Self::offload_lookup(host, port).await
        }

        #[cfg(target_family = "wasm")]
        {
            Self::lookup_blocking(&host, port)
        }
    }

    fn resolve_blocking(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, DnsError> {
        if host.is_empty() {
            return Err(DnsError::InvalidHost(host.to_string()));
        }

        // The blocking boundary already owns a plain thread, so resolve inline
        // instead of submitting to the valtron background registry (which may
        // not be initialized here and is not needed off the worker).
        Self::lookup_blocking(host, port)
    }
}

/// Cached DNS entry.
#[derive(Debug, Clone)]
struct CachedEntry {
    addresses: Vec<SocketAddr>,
    expires_at: Instant,
}

impl CachedEntry {
    /// Checks if this entry has expired on `clock`.
    fn is_expired(&self, clock: &dyn foundation_core::valtron::Clock) -> bool {
        clock.now() >= self.expires_at
    }
}

/// Caching DNS resolver that wraps another resolver.
///
/// Caches DNS resolution results with a configurable TTL.
///
/// # Type Parameters
///
/// * `R` - The inner resolver type
#[derive(Clone)]
pub struct CachingDnsResolver<R: DnsResolver> {
    inner: R,
    cache: Arc<Mutex<HashMap<String, CachedEntry>>>,
    ttl: Duration,
    /// The clock that stamps and expires cache entries.
    clock: Arc<dyn foundation_core::valtron::Clock>,
}

impl<R: DnsResolver> core::fmt::Debug for CachingDnsResolver<R> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CachingDnsResolver")
            .field("ttl", &self.ttl)
            .finish_non_exhaustive()
    }
}

impl<R: DnsResolver> CachingDnsResolver<R> {
    /// Creates a new caching DNS resolver with the given TTL.
    ///
    /// # Arguments
    ///
    /// * `inner` - The resolver to wrap
    /// * `ttl` - Time-to-live for cache entries
    pub fn new(inner: R, ttl: Duration, clock: Arc<dyn foundation_core::valtron::Clock>) -> Self {
        Self {
            inner,
            cache: Arc::new(Mutex::new(HashMap::new())),
            ttl,
            clock,
        }
    }

    /// Creates a new caching resolver with default 5-minute TTL.
    pub fn with_default_ttl(inner: R, clock: Arc<dyn foundation_core::valtron::Clock>) -> Self {
        Self::new(inner, Duration::from_secs(300), clock)
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

#[async_trait::async_trait]
impl<R: DnsResolver> DnsResolver for CachingDnsResolver<R> {
    async fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, DnsError> {
        let cache_key = format!("{host}:{port}");

        // Check cache first
        if let Ok(cache) = self.cache.lock() {
            if let Some(entry) = cache.get(&cache_key) {
                if !entry.is_expired(&*self.clock) {
                    return Ok(entry.addresses.clone());
                }
            }
        }

        // Cache miss or expired - resolve using inner resolver
        let addresses = self.inner.resolve(host, port).await?;

        // Store in cache
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(
                cache_key,
                CachedEntry {
                    addresses: addresses.clone(),
                    expires_at: self.clock.now() + self.ttl,
                },
            );
        }

        Ok(addresses)
    }

    fn resolve_blocking(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, DnsError> {
        let cache_key = format!("{host}:{port}");

        // Check cache first
        if let Ok(cache) = self.cache.lock() {
            if let Some(entry) = cache.get(&cache_key) {
                if !entry.is_expired(&*self.clock) {
                    return Ok(entry.addresses.clone());
                }
            }
        }

        // Cache miss or expired - resolve the inner resolver synchronously, so a
        // `CachingDnsResolver<SystemDnsResolver>` does not submit a background job
        // on a plain thread.
        let addresses = self.inner.resolve_blocking(host, port)?;

        // Store in cache
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(
                cache_key,
                CachedEntry {
                    addresses: addresses.clone(),
                    expires_at: self.clock.now() + self.ttl,
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

#[async_trait::async_trait]
impl DnsResolver for MockDnsResolver {
    async fn resolve(&self, host: &str, _port: u16) -> Result<Vec<SocketAddr>, DnsError> {
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
