//! Subscription configuration.
//!
//! Every size, timeout and retry value is a field with a builder method and a documented
//! default. The code path never uses a bare constant.

/// The subscription configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubscriptionConfig {
    /// The stream-root-relative prefix of every stream, without a leading or trailing slash.
    ///
    /// The reference server mounts streams under `v1/stream`. The handler also uses it to find
    /// the reserved control prefix and the stored subscription state.
    pub stream_root: String,
    /// The reserved control segment. The protocol fixes it to `__ds`.
    pub control_segment: String,
    /// The fallback lease time to live (TTL) in milliseconds. Default 30 000, the protocol
    /// default of 30 seconds.
    pub default_lease_ttl_ms: u64,
    /// The shortest accepted lease time to live (TTL) in milliseconds. Default 1 000.
    pub min_lease_ttl_ms: u64,
    /// The longest accepted lease time to live (TTL) in milliseconds. Default 600 000, the
    /// protocol maximum of 10 minutes.
    pub max_lease_ttl_ms: u64,
    /// The first webhook retry delay in milliseconds. Default 1 000, the protocol minimum.
    pub min_retry_delay_ms: u64,
    /// The longest webhook retry delay in milliseconds. Default 60 000, the protocol maximum.
    pub max_retry_delay_ms: u64,
    /// The retry jitter in percent of the delay. Default 20, the protocol value.
    pub retry_jitter_percent: u32,
    /// The largest subscription request body in bytes.
    ///
    /// Default 1 MiB: the nginx `client_max_body_size` default. Subscription bodies are small,
    /// so this bounds a hostile request without rejecting a real one.
    pub max_body_bytes: usize,
    /// The largest webhook response body read for the `done` flag, in bytes.
    ///
    /// Default 64 KiB, the same cap as the platform's error-body reader.
    pub max_webhook_response_bytes: usize,
    /// The public base uniform resource locator (URL) of the server, for callback and JWKS
    /// links. `None` derives it from the `Host` header of the create request.
    pub public_base_url: Option<String>,
    /// The scheme used when [`Self::public_base_url`] is `None`. Default `http`.
    pub public_scheme: String,
    /// The longest webhook connect wait in milliseconds. Default 10 000.
    pub webhook_connect_timeout_ms: u64,
    /// The longest webhook read wait in milliseconds. Default 30 000.
    pub webhook_read_timeout_ms: u64,
}

impl SubscriptionConfig {
    /// Build a configuration for a stream root such as `v1/stream`.
    #[must_use]
    pub fn new(stream_root: impl Into<String>) -> Self {
        Self {
            stream_root: normalize_relative_path(&stream_root.into()),
            control_segment: "__ds".to_string(),
            default_lease_ttl_ms: 30_000,
            min_lease_ttl_ms: 1_000,
            max_lease_ttl_ms: 600_000,
            min_retry_delay_ms: 1_000,
            max_retry_delay_ms: 60_000,
            retry_jitter_percent: 20,
            max_body_bytes: 1024 * 1024,
            max_webhook_response_bytes: 64 * 1024,
            public_base_url: None,
            public_scheme: "http".to_string(),
            webhook_connect_timeout_ms: 10_000,
            webhook_read_timeout_ms: 30_000,
        }
    }

    /// Set the reserved control segment.
    #[must_use]
    pub fn with_control_segment(mut self, segment: impl Into<String>) -> Self {
        self.control_segment = segment.into();
        self
    }

    /// Set the fallback lease time to live (TTL) in milliseconds.
    #[must_use]
    pub fn with_default_lease_ttl_ms(mut self, milliseconds: u64) -> Self {
        self.default_lease_ttl_ms = milliseconds;
        self
    }

    /// Set the accepted lease time to live (TTL) range in milliseconds.
    #[must_use]
    pub fn with_lease_ttl_range_ms(mut self, min: u64, max: u64) -> Self {
        self.min_lease_ttl_ms = min;
        self.max_lease_ttl_ms = max;
        self
    }

    /// Set the webhook retry delay range in milliseconds.
    #[must_use]
    pub fn with_retry_delay_range_ms(mut self, min: u64, max: u64) -> Self {
        self.min_retry_delay_ms = min;
        self.max_retry_delay_ms = max;
        self
    }

    /// Set the retry jitter in percent of the delay.
    #[must_use]
    pub fn with_retry_jitter_percent(mut self, percent: u32) -> Self {
        self.retry_jitter_percent = percent;
        self
    }

    /// Set the largest subscription request body in bytes.
    #[must_use]
    pub fn with_max_body_bytes(mut self, bytes: usize) -> Self {
        self.max_body_bytes = bytes;
        self
    }

    /// Set the largest webhook response body read for the `done` flag.
    #[must_use]
    pub fn with_max_webhook_response_bytes(mut self, bytes: usize) -> Self {
        self.max_webhook_response_bytes = bytes;
        self
    }

    /// Set the public base uniform resource locator (URL) of the server.
    #[must_use]
    pub fn with_public_base_url(mut self, base: impl Into<String>) -> Self {
        self.public_base_url = Some(base.into());
        self
    }

    /// Set the scheme used when the public base uniform resource locator (URL) is derived.
    #[must_use]
    pub fn with_public_scheme(mut self, scheme: impl Into<String>) -> Self {
        self.public_scheme = scheme.into();
        self
    }

    /// Set the webhook connect and read waits in milliseconds.
    #[must_use]
    pub fn with_webhook_timeouts_ms(mut self, connect: u64, read: u64) -> Self {
        self.webhook_connect_timeout_ms = connect;
        self.webhook_read_timeout_ms = read;
        self
    }

    /// The reserved control prefix in stream-name form, such as `v1/stream/__ds`.
    #[must_use]
    pub fn control_prefix(&self) -> String {
        format!("{}/{}", self.stream_root, self.control_segment)
    }

    /// The subscription state prefix in stream-name form.
    #[must_use]
    pub fn state_prefix(&self) -> String {
        format!("{}/state", self.control_prefix())
    }

    /// The absolute path of the JSON Web Key Set (JWKS) endpoint.
    #[must_use]
    pub fn jwks_path(&self) -> String {
        format!("/{}/jwks.json", self.control_prefix())
    }

    /// The absolute path of a subscription control resource.
    #[must_use]
    pub fn subscription_path(&self, id: &str) -> String {
        format!("/{}/subscriptions/{id}", self.control_prefix())
    }

    /// Return the stream-root-relative path for an absolute stream name.
    ///
    /// Returns `None` for a name outside the stream root or inside the reserved control prefix.
    #[must_use]
    pub fn relative_path<'a>(&self, name: &'a str) -> Option<&'a str> {
        let rest = name.strip_prefix(self.stream_root.as_str())?;
        let rest = rest.strip_prefix('/')?;
        if rest.is_empty() || rest == self.control_segment {
            return None;
        }
        if let Some(tail) = rest.strip_prefix(self.control_segment.as_str()) {
            if tail.starts_with('/') {
                return None;
            }
        }
        Some(rest)
    }

    /// Return the absolute stream name for a stream-root-relative path.
    #[must_use]
    pub fn absolute_path(&self, relative: &str) -> String {
        format!("{}/{}", self.stream_root, normalize_relative_path(relative))
    }
}

impl Default for SubscriptionConfig {
    fn default() -> Self {
        Self::new("v1/stream")
    }
}

/// Trim the leading and trailing slashes from a stream path.
#[must_use]
pub fn normalize_relative_path(path: &str) -> String {
    path.trim_matches('/').to_string()
}
