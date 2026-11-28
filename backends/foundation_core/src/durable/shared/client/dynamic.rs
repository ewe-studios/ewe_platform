//! Per-request header and query values.
//!
//! A provider supplies values that change between requests, such as a refreshed
//! OAuth token or a request counter. The client calls it while it builds each
//! request, so it runs once per attempt and never caches a stale value.

/// Supplies request headers and query parameters for one attempt.
///
/// The client evaluates the provider when it builds a request. A retry builds a
/// new request, so the provider runs again and its values can change.
///
/// Implementations must be thread-safe: the client can share one provider
/// across concurrent reads.
pub trait DynamicHeaders: Send + Sync {
    /// The headers for the request that is about to run.
    fn headers(&self) -> Vec<(String, String)>;

    /// The query parameters for the request that is about to run.
    ///
    /// The default supplies none.
    fn query(&self) -> Vec<(String, String)> {
        Vec::new()
    }
}
