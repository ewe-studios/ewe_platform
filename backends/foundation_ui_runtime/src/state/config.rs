use core::time::Duration;

use flatbuffers::FLATBUFFERS_MAX_BUFFER_SIZE;
use foundation_ui_wire::WireLimits;

// Upper bound for the state table, vector headers, padding, binding, and tag.
// This also includes the nonce and authentication tag of private state.
const ENVELOPE_OVERHEAD: usize = 256;

/// The seal lifetime and maximum plaintext body size.
///
/// The default lifetime is 24 hours. The default body limit is 4096 bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateConfig {
    ttl: Duration,
    body_bytes: usize,
}

impl Default for StateConfig {
    fn default() -> Self {
        Self {
            ttl: Duration::from_hours(24),
            body_bytes: 4096,
        }
    }
}

impl StateConfig {
    #[must_use]
    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    /// Sets the plaintext limit, up to the buffer capacity after metadata.
    #[must_use]
    pub fn max_body_bytes(mut self, bytes: usize) -> Self {
        self.body_bytes = bytes.min(FLATBUFFERS_MAX_BUFFER_SIZE - ENVELOPE_OVERHEAD);
        self
    }

    #[must_use]
    pub fn ttl_value(&self) -> Duration {
        self.ttl
    }

    #[must_use]
    pub fn body_bytes(&self) -> usize {
        self.body_bytes
    }

    pub(super) fn wire_limits(&self) -> WireLimits {
        let max_bytes = self.body_bytes.saturating_add(ENVELOPE_OVERHEAD);
        WireLimits::default().max_action_request_bytes(max_bytes)
    }
}

/// The number of failures, measurement window, and block time.
///
/// The defaults are five failures, a 600-second window, and a 600-second block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FailureLimits {
    failures: u32,
    window: Duration,
    block_for: Duration,
}

impl Default for FailureLimits {
    fn default() -> Self {
        Self {
            failures: 5,
            window: Duration::from_secs(600),
            block_for: Duration::from_secs(600),
        }
    }
}

impl FailureLimits {
    #[must_use]
    pub fn max_failures(mut self, failures: u32) -> Self {
        self.failures = failures;
        self
    }

    #[must_use]
    pub fn window(mut self, window: Duration) -> Self {
        self.window = window;
        self
    }

    #[must_use]
    pub fn block_for(mut self, duration: Duration) -> Self {
        self.block_for = duration;
        self
    }

    #[must_use]
    pub fn failure_count(&self) -> u32 {
        self.failures
    }

    #[must_use]
    pub fn window_value(&self) -> Duration {
        self.window
    }

    #[must_use]
    pub fn block_duration(&self) -> Duration {
        self.block_for
    }
}
