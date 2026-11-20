use core::time::Duration;

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

    /// Sets the plaintext limit, in bytes.
    #[must_use]
    pub fn max_body_bytes(mut self, bytes: usize) -> Self {
        self.body_bytes = bytes;
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
}
