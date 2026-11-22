//! The Transport Layer Security (TLS) time source.

use std::sync::Arc;

use foundation_context::Clock;
use rustls::pki_types::UnixTime;
use rustls::time_provider::TimeProvider;

/// A `rustls` time source that reads the `Ctx` clock.
///
/// `rustls` asks for the current time when it checks a validity window. A
/// deterministic simulation test (DST) run must cross that window at a known
/// point, so the time comes from the injected clock, never from the system
/// clock.
#[derive(Clone)]
pub struct CtxTimeProvider(pub Arc<dyn Clock>);

impl std::fmt::Debug for CtxTimeProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CtxTimeProvider(<ctx clock>)")
    }
}

impl TimeProvider for CtxTimeProvider {
    fn current_time(&self) -> Option<UnixTime> {
        // `unix_seconds` returns a negative value before the epoch, and
        // `rustls` has no time before the epoch. `None` makes `rustls` reject
        // the certificate.
        let secs = foundation_compact::time::unix_seconds(self.0.wall_now());
        let secs = u64::try_from(secs).ok()?;
        Some(UnixTime::since_unix_epoch(core::time::Duration::from_secs(
            secs,
        )))
    }
}
