//! Transport-neutral cache and collapsing-cursor helpers.
//!
//! The HTTP adapter formats these values. They have no HTTP type.

/// The ETag for a read range.
///
/// The value changes when the stream closes, even when no new bytes arrived. A stale
/// `If-None-Match` therefore cannot hide the closure signal.
#[must_use]
pub fn etag(stream: &str, start: u64, end: u64, closed: bool) -> String {
    let suffix = if closed { ":c" } else { "" };
    format!("\"{stream}:{start}:{end}{suffix}\"")
}

/// The cache policy for one response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CachePolicy {
    /// A shared, non-user-specific stream.
    Public,
    /// A user-specific stream.
    Private,
    /// A response that must not be stored.
    NoStore,
    /// A live response that must revalidate.
    NoCache,
}

impl CachePolicy {
    /// Return the `Cache-Control` value.
    #[must_use]
    pub fn header_value(self) -> &'static str {
        match self {
            Self::Public => "public, max-age=60, stale-while-revalidate=300",
            Self::Private => "private, max-age=60, stale-while-revalidate=300",
            Self::NoStore => "no-store",
            Self::NoCache => "no-cache",
        }
    }
}

/// The collapsing-cursor configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorConfig {
    /// The interval in seconds. Default 20.
    pub interval_secs: u64,
    /// The epoch in UNIX seconds. Default 2024-10-09T00:00:00Z.
    pub epoch_unix_secs: u64,
}

impl Default for CursorConfig {
    fn default() -> Self {
        Self {
            interval_secs: 20,
            epoch_unix_secs: 1_728_432_000,
        }
    }
}

/// Generate a collapsing cursor.
///
/// The value is the interval number. If the client cursor is at or ahead, the result is strictly
/// greater. `jitter_secs` comes from the injected random source and must be 1 to 3600.
#[must_use]
pub fn generate_cursor(
    client: Option<u64>,
    now_unix_secs: u64,
    config: &CursorConfig,
    jitter_secs: u64,
) -> u64 {
    let interval = now_unix_secs
        .saturating_sub(config.epoch_unix_secs)
        .checked_div(config.interval_secs.max(1))
        .unwrap_or(0);

    match client {
        Some(client) if client >= interval => {
            let jitter = jitter_secs.clamp(1, 3600);
            client.saturating_add(1).saturating_add(jitter)
        }
        _ => interval,
    }
}
