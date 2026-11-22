//! How the trust store keeps its in-memory index, and its limits.

use foundation_compact::Duration;

/// How the in-memory index works.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum IndexMode {
    /// Load every client at open. The Transport Layer Security (TLS) verifier
    /// reads it with no Input/Output (I/O). This is the default for one native
    /// process.
    #[default]
    Preload,
    /// Like [`IndexMode::Preload`], and reload every
    /// [`TrustConfig::cache_ttl`]. Use it for native replicas that share a
    /// database.
    Refresh,
    /// No index: every check reads the store. Use it for a Cloudflare Worker,
    /// whose isolates are short-lived and many.
    StoreOnly,
}

/// The limits and intervals of the trust store.
///
/// Every value is a field with a `with_` builder and a documented default.
#[derive(Debug, Clone)]
pub struct TrustConfig {
    /// How the in-memory index works.
    pub index: IndexMode,
    /// Write `last_seen_at` at most once per interval per client. Default 75
    /// seconds, close to nginx's default `keepalive_timeout`, so one idle
    /// keep-alive connection makes at most one write.
    pub touch_interval: Duration,
    /// The longest time that the in-memory index may be stale. Default 5
    /// minutes, shorter than one enrollment window.
    pub cache_ttl: Duration,
    /// The longest time that a revocation decision may be stale. Default 1
    /// hour: a compromise is rare, and one hour bounds the exposure.
    pub revocation_ttl: Duration,
    /// How often `prune` runs. Default 1 hour.
    pub prune_interval: Duration,
    /// Keep a use and revocation row this long after its secret expires.
    /// Default 5 minutes, equal to the secret clock skew.
    pub expired_grace: Duration,
    /// Other processes write the same store.
    pub shared_store: bool,
}

impl Default for TrustConfig {
    fn default() -> Self {
        Self {
            index: IndexMode::Preload,
            touch_interval: Duration::from_secs(75),
            cache_ttl: Duration::from_mins(5),
            revocation_ttl: Duration::from_hours(1),
            prune_interval: Duration::from_hours(1),
            expired_grace: Duration::from_mins(5),
            shared_store: false,
        }
    }
}

impl TrustConfig {
    /// Set the index mode.
    #[must_use]
    pub fn with_index(mut self, index: IndexMode) -> Self {
        self.index = index;
        self
    }

    /// Set the last-seen write interval.
    #[must_use]
    pub fn with_touch_interval(mut self, interval: Duration) -> Self {
        self.touch_interval = interval;
        self
    }

    /// Set the trust store cache time.
    #[must_use]
    pub fn with_cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = ttl;
        self
    }

    /// Set the revocation check period.
    #[must_use]
    pub fn with_revocation_ttl(mut self, ttl: Duration) -> Self {
        self.revocation_ttl = ttl;
        self
    }

    /// Set the prune interval.
    #[must_use]
    pub fn with_prune_interval(mut self, interval: Duration) -> Self {
        self.prune_interval = interval;
        self
    }

    /// Set the grace period after a secret expires.
    #[must_use]
    pub fn with_expired_grace(mut self, grace: Duration) -> Self {
        self.expired_grace = grace;
        self
    }

    /// Say whether other processes write the same store.
    #[must_use]
    pub fn with_shared_store(mut self, shared: bool) -> Self {
        self.shared_store = shared;
        self
    }
}
