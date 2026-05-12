//! Dynamic timeout calculation system for HTTP client and server.
//!
//! This module provides production-quality timeout calculations based on:
//! - Body size (sub-linear scaling using sqrt)
//! - Network conditions
//! - Historical latency data
//! - Endpoint characteristics
//!
//! # Example
//!
//! ```
//! use foundation_core::wire::simple_http::timeout::{TimeoutCalculator, TimeoutContext};
//! use std::time::Duration;
//!
//! let calculator = TimeoutCalculator::new();
//! let context = TimeoutContext::with_size(1024); // 1KB body
//! let timeout = calculator.calculate_read_timeout(&context);
//! ```

use crate::wire::simple_http::client_classifier::ClientClassifier;
use crate::wire::simple_http::latency_tracker::LatencyTracker;
use crate::wire::simple_http::load_tracker::LoadTracker;
use std::time::Duration;

/// Configuration for timeout calculations.
///
/// WHY: Production systems need configurable timeouts to handle varying
/// network conditions and payload sizes appropriately.
///
/// WHAT: Defines base timeout values, bounds, and retry configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeoutConfig {
    /// TCP connection establishment timeout.
    pub connect_timeout: Duration,

    /// Base read timeout per KB of expected body size.
    /// Scaled sub-linearly using sqrt(size_kb) to avoid excessive timeouts.
    pub read_timeout_per_kb: Duration,

    /// Base write timeout per KB of body size for uploads.
    pub write_timeout_per_kb: Duration,

    /// Minimum read timeout regardless of body size.
    pub min_read_timeout: Duration,

    /// Maximum read timeout regardless of body size.
    pub max_read_timeout: Duration,

    /// Maximum total request timeout (includes all retries).
    pub max_total_timeout: Duration,

    /// Time-to-first-byte timeout for initial server response.
    pub ttfb_timeout: Duration,

    /// Maximum number of retry attempts for transient failures.
    pub max_retries: usize,

    /// Minimum sleep duration between work iterations (default: 15ms).
    /// Used by `calculate_sleep_duration` for HTTP polling.
    pub min_sleep_duration: Duration,

    /// Maximum sleep duration between work iterations (default: 100ms).
    /// Used by `calculate_sleep_duration` as upper clamp.
    pub max_sleep_duration: Duration,

    /// Sleep fraction divisor for timeout-based calculation (default: 100 = 1%).
    /// `sleep_ms = timeout_ms / sleep_timeout_fraction`.
    pub sleep_timeout_fraction: u64,
}

impl Default for TimeoutConfig {
    /// Returns production-quality default timeout configuration.
    ///
    /// # Values
    ///
    /// | Field | Default | Rationale |
    /// |-------|---------|-----------|
    /// | connect_timeout | 10s | TCP handshake completion |
    /// | read_timeout_per_kb | 10ms | Base for size-based calculation |
    /// | write_timeout_per_kb | 5ms | Upload speed factor |
    /// | min_read_timeout | 100ms | Absolute minimum for small payloads |
    /// | max_read_timeout | 60s | Prevents excessive waits |
    /// | max_total_timeout | 300s | Maximum overall request time |
    /// | ttfb_timeout | 5s | Server response latency |
    /// | max_retries | 3 | Balance reliability vs latency |
    /// | min_sleep_duration | 15ms | Base sleep for HTTP polling |
    /// | max_sleep_duration | 100ms | Upper clamp for calculated sleep |
    /// | sleep_timeout_fraction | 100 | 1% of timeout for calculated sleep |
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            read_timeout_per_kb: Duration::from_millis(10),
            write_timeout_per_kb: Duration::from_millis(5),
            min_read_timeout: Duration::from_millis(100),
            max_read_timeout: Duration::from_secs(60),
            max_total_timeout: Duration::from_secs(300),
            ttfb_timeout: Duration::from_secs(5),
            max_retries: 3,
            min_sleep_duration: Duration::from_millis(15),
            max_sleep_duration: Duration::from_millis(100),
            sleep_timeout_fraction: 100,
        }
    }
}

/// Context for timeout calculation.
///
/// WHY: Different requests need different timeouts based on body size,
/// endpoint characteristics, and operation type.
///
/// WHAT: Encapsulates all contextual information needed for timeout calculation.
///
/// HOW: Created per-request and passed to TimeoutCalculator methods.
#[derive(Debug, Clone, Default)]
pub struct TimeoutContext {
    /// Target endpoint (e.g., "api.example.com/v1/users").
    pub endpoint: Option<String>,

    /// Expected body size in bytes (None if unknown).
    pub expected_body_size: Option<usize>,

    /// Whether this is an upload operation (POST/PUT with body).
    pub is_upload: bool,

    /// Whether this is a streaming response.
    pub is_streaming: bool,

    /// Previous timeout duration for exponential backoff calculation.
    /// When provided, the calculator can use this to compute the next timeout
    /// in a retry sequence (e.g., doubling for exponential backoff).
    pub previous_timeout: Option<Duration>,
}

impl TimeoutContext {
    /// Create a context with only body size specified.
    #[must_use]
    pub fn with_size(size: usize) -> Self {
        Self {
            expected_body_size: Some(size),
            ..Self::default()
        }
    }

    /// Create a context for a specific endpoint.
    #[must_use]
    pub fn with_endpoint(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: Some(endpoint.into()),
            ..Self::default()
        }
    }

    /// Set the expected body size.
    #[must_use]
    pub fn with_body_size(mut self, size: usize) -> Self {
        self.expected_body_size = Some(size);
        self
    }

    /// Mark as upload operation.
    #[must_use]
    pub fn upload(mut self) -> Self {
        self.is_upload = true;
        self
    }

    /// Mark as streaming operation.
    #[must_use]
    pub fn streaming(mut self) -> Self {
        self.is_streaming = true;
        self
    }

    /// Set previous timeout for exponential backoff calculation.
    ///
    /// WHY: When retrying requests, the previous timeout can inform
    /// the next timeout calculation (e.g., exponential backoff).
    ///
    /// WHAT: Builder method to set the previous timeout duration.
    ///
    /// # Example
    ///
    /// ```
    /// use foundation_core::wire::simple_http::timeout::TimeoutContext;
    /// use std::time::Duration;
    ///
    /// let ctx = TimeoutContext::with_size(1024)
    ///     .with_previous_timeout(Duration::from_secs(1));
    /// ```
    #[must_use]
    pub fn with_previous_timeout(mut self, timeout: Duration) -> Self {
        self.previous_timeout = Some(timeout);
        self
    }
}

/// Production-quality timeout calculator.
///
/// WHY: Fixed timeouts don't scale with body size or network conditions.
/// This calculator adapts timeouts based on payload size using sub-linear scaling
/// and historical latency data.
///
/// WHAT: Calculates timeouts for HTTP operations using size-based formulas
/// with configurable bounds and optional latency adjustment.
///
/// HOW: Uses sqrt(size_kb) scaling to provide reasonable timeouts for both
/// small API calls and large file transfers. Optionally uses LatencyTracker
/// to adjust based on historical endpoint behavior.
///
/// # Example
///
/// ```
/// use foundation_core::wire::simple_http::timeout::{TimeoutCalculator, TimeoutContext};
/// use std::time::Duration;
///
/// let calc = TimeoutCalculator::new();
///
/// // Small API call: ~100ms
/// let ctx = TimeoutContext::with_size(1024); // 1KB
/// let timeout = calc.calculate_read_timeout(&ctx);
/// assert!(timeout >= Duration::from_millis(100));
///
/// // Large file: ~1s
/// let ctx = TimeoutContext::with_size(1024 * 1024); // 1MB
/// let timeout = calc.calculate_read_timeout(&ctx);
/// assert!(timeout >= Duration::from_millis(300));
/// ```
#[derive(Debug, Clone)]
pub struct TimeoutCalculator {
    config: TimeoutConfig,
    /// Optional latency tracker for adaptive timeouts.
    latency_tracker: Option<LatencyTracker>,
    /// Optional load tracker for load-based scaling.
    load_tracker: Option<LoadTracker>,
    /// Optional client classifier for DoS protection.
    client_classifier: Option<ClientClassifier>,
    /// Factor to apply to P99 latency as safety margin (default: 2.0).
    latency_safety_factor: f64,
}

impl TimeoutCalculator {
    /// Create a new calculator with production default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: TimeoutConfig::default(),
            latency_tracker: None,
            load_tracker: None,
            client_classifier: None,
            latency_safety_factor: 2.0,
        }
    }

    /// Create a calculator with custom configuration.
    #[must_use]
    pub fn with_config(config: TimeoutConfig) -> Self {
        Self {
            config,
            latency_tracker: None,
            load_tracker: None,
            client_classifier: None,
            latency_safety_factor: 2.0,
        }
    }

    /// Create a calculator with latency tracking enabled.
    #[must_use]
    pub fn with_latency_tracking(config: TimeoutConfig, tracker: LatencyTracker) -> Self {
        Self {
            config,
            latency_tracker: Some(tracker),
            load_tracker: None,
            client_classifier: None,
            latency_safety_factor: 2.0,
        }
    }

    /// Create a calculator with load tracking enabled.
    #[must_use]
    pub fn with_load_tracking(config: TimeoutConfig, tracker: LoadTracker) -> Self {
        Self {
            config,
            latency_tracker: None,
            load_tracker: Some(tracker),
            client_classifier: None,
            latency_safety_factor: 2.0,
        }
    }

    /// Enable latency tracking with default config.
    #[must_use]
    pub fn enable_latency_tracking(mut self) -> Self {
        self.latency_tracker = Some(LatencyTracker::new());
        self
    }

    /// Enable load tracking with default config.
    #[must_use]
    pub fn enable_load_tracking(mut self) -> Self {
        self.load_tracker = Some(LoadTracker::new());
        self
    }

    /// Enable client classification for DoS protection.
    #[must_use]
    pub fn with_client_classifier(mut self, classifier: ClientClassifier) -> Self {
        self.client_classifier = Some(classifier);
        self
    }

    /// Enable client classification with default config.
    #[must_use]
    pub fn enable_client_classification(mut self) -> Self {
        self.client_classifier = Some(ClientClassifier::new());
        self
    }

    /// Get the load tracker if enabled.
    #[must_use]
    pub fn load_tracker(&self) -> Option<&LoadTracker> {
        self.load_tracker.as_ref()
    }

    /// Get the client classifier if enabled.
    #[must_use]
    pub fn client_classifier(&self) -> Option<&ClientClassifier> {
        self.client_classifier.as_ref()
    }

    /// Set the latency safety factor.
    ///
    /// The calculated timeout is: base_timeout + (p99_latency * factor)
    #[must_use]
    pub fn with_latency_safety_factor(mut self, factor: f64) -> Self {
        self.latency_safety_factor = factor;
        self
    }

    /// Get the latency tracker if enabled.
    #[must_use]
    pub fn latency_tracker(&self) -> Option<&LatencyTracker> {
        self.latency_tracker.as_ref()
    }

    /// Get the configuration.
    #[must_use]
    pub fn config(&self) -> &TimeoutConfig {
        &self.config
    }

    /// Record a latency sample for an endpoint.
    ///
    /// Does nothing if latency tracking is not enabled.
    pub fn record_latency(&self, endpoint: &str, duration: Duration) {
        if let Some(ref tracker) = self.latency_tracker {
            tracker.record(endpoint, duration);
        }
    }

    /// Record a transfer for client classification.
    ///
    /// WHY: Server needs to track transfer rates per client IP to classify
    /// clients for DoS protection and apply appropriate timeouts.
    ///
    /// Does nothing if client classification is not enabled.
    pub fn record_client_transfer(&self, client_ip: &str, bytes: usize, duration: Duration) {
        if let Some(ref classifier) = self.client_classifier {
            classifier.record_transfer(client_ip, bytes, duration);
        }
    }

    /// Get classification for a client IP.
    ///
    /// Returns None if client classification is not enabled.
    #[must_use]
    pub fn classify_client(&self, client_ip: &str) -> Option<crate::wire::simple_http::client_classifier::ClientClassification> {
        self.client_classifier.as_ref().map(|c| c.classify(client_ip))
    }

    /// Calculate read timeout for a request.
    ///
    /// Uses size-based calculation with optional latency adjustment.
    #[must_use]
    pub fn calculate_read_timeout(&self, ctx: &TimeoutContext) -> Duration {
        let base_timeout = if let Some(size) = ctx.expected_body_size {
            self.size_based_read_timeout(size)
        } else {
            self.config.min_read_timeout
        };

        // Apply latency adjustment if endpoint is known and we have stats
        let adjusted_timeout = if let Some(ref endpoint) = ctx.endpoint {
            if let Some(ref tracker) = self.latency_tracker {
                if let Some(stats) = tracker.get_stats(endpoint) {
                    // Add P99 latency * safety factor as safety margin
                    let latency_adjustment = Duration::from_millis(
                        (stats.p99.as_millis() as f64 * self.latency_safety_factor) as u64,
                    );
                    base_timeout + latency_adjustment
                } else {
                    base_timeout
                }
            } else {
                base_timeout
            }
        } else {
            base_timeout
        };

        // Apply load-based scaling if enabled
        let adjusted_timeout = if let Some(ref load_tracker) = self.load_tracker {
            let factor = load_tracker.timeout_factor();
            Duration::from_millis((adjusted_timeout.as_millis() as f64 * factor) as u64)
        } else {
            adjusted_timeout
        };

        // Apply exponential backoff if previous_timeout is provided (retry scenario)
        let adjusted_timeout = if let Some(prev) = ctx.previous_timeout {
            // Double the previous timeout for exponential backoff
            // Cap at max_read_timeout
            let doubled = prev * 2;
            adjusted_timeout.max(doubled)
        } else {
            adjusted_timeout
        };

        // Clamp to bounds
        adjusted_timeout.clamp(self.config.min_read_timeout, self.config.max_read_timeout)
    }

    /// Calculate read timeout with client IP for classification.
    ///
    /// WHY: Server connections need to classify clients by IP for DoS protection
    /// and apply appropriate timeout multipliers based on transfer rate history.
    ///
    /// WHAT: Calculates read timeout with optional client classification multiplier.
    ///
    /// HOW: Applies client-specific multiplier if client_classifier is configured
    /// and client_ip is provided.
    #[must_use]
    pub fn calculate_read_timeout_with_client(
        &self,
        ctx: &TimeoutContext,
        client_ip: Option<&str>,
    ) -> Duration {
        let base_timeout = self.calculate_read_timeout(ctx);

        // Apply client classification multiplier if available
        if let Some(ref classifier) = self.client_classifier {
            if let Some(ip) = client_ip {
                let multiplier = classifier.timeout_multiplier(ip);
                return Duration::from_millis((base_timeout.as_millis() as f64 * multiplier) as u64);
            }
        }

        base_timeout
    }

    /// Calculate write timeout for a request.
    ///
    /// Similar to read timeout but uses write_timeout_per_kb.
    /// Uploads get additional scaling factor.
    #[must_use]
    pub fn calculate_write_timeout(&self, ctx: &TimeoutContext) -> Duration {
        let base_timeout = if let Some(size) = ctx.expected_body_size {
            let size_kb = size as f64 / 1024.0;
            let per_kb_ms = self.config.write_timeout_per_kb.as_millis() as f64;
            let scaled_ms = per_kb_ms * size_kb.sqrt();

            if ctx.is_upload {
                // Uploads are typically slower, apply 1.5x factor
                Duration::from_millis((scaled_ms * 1.5) as u64)
            } else {
                Duration::from_millis(scaled_ms as u64)
            }
        } else {
            self.config.min_read_timeout
        };

        // Clamp to bounds (use read bounds for write too)
        base_timeout.clamp(self.config.min_read_timeout, self.config.max_read_timeout)
    }

    /// Calculate total request timeout.
    ///
    /// This is the maximum time for the entire request including retries.
    /// Uses max_total_timeout as upper bound.
    #[must_use]
    pub fn calculate_total_timeout(&self, ctx: &TimeoutContext) -> Duration {
        let read_timeout = self.calculate_read_timeout(ctx);
        let retry_count = self.config.max_retries as u64;

        // Total = read_timeout * (retries + 1) for initial attempt
        let total = read_timeout * (retry_count + 1).try_into().unwrap_or(u32::MAX);

        total.min(self.config.max_total_timeout)
    }

    /// Calculate size-based read timeout.
    ///
    /// Formula: `timeout_ms = read_timeout_per_kb_ms * sqrt(size_kb)`
    ///
    /// Examples:
    /// - 1KB: sqrt(1) * 10ms = 10ms
    /// - 10KB: sqrt(10) * 10ms = 32ms
    /// - 1MB: sqrt(1024) * 10ms = 320ms
    /// - 100MB: sqrt(102400) * 10ms = 3.2s
    fn size_based_read_timeout(&self, size_bytes: usize) -> Duration {
        if size_bytes == 0 {
            return self.config.min_read_timeout;
        }

        let size_kb = size_bytes as f64 / 1024.0;
        let per_kb_ms = self.config.read_timeout_per_kb.as_millis() as f64;

        // Sub-linear scaling: sqrt(size_kb) * per_kb
        let scaled_ms = per_kb_ms * size_kb.sqrt();

        Duration::from_millis(scaled_ms as u64)
    }

    /// Calculate sleep duration between work iterations.
    ///
    /// WHY: Different operations need different polling intervals.
    /// Fast operations (HTTP requests) need shorter sleep.
    /// Streaming operations (WebSocket, SSE) need longer sleep to reduce CPU usage.
    ///
    /// WHAT: Returns appropriate sleep duration based on operation type and timeout context.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::timeout::{TimeoutCalculator, TimeoutContext};
    ///
    /// let calc = TimeoutCalculator::new();
    ///
    /// // For HTTP polling: ~15ms
    /// let http_sleep = calc.calculate_sleep_duration(&TimeoutContext::default());
    ///
    /// // For streaming with known body: scales with timeout
    /// let streaming_ctx = TimeoutContext::with_size(1024 * 1024).streaming();
    /// let streaming_sleep = calc.calculate_sleep_duration(&streaming_ctx);
    /// ```
    #[must_use]
    pub fn calculate_sleep_duration(&self, ctx: &TimeoutContext) -> Duration {
        // Base sleep duration from config
        let base_sleep_ms = self.config.min_sleep_duration.as_millis() as u64;

        // For streaming operations, use longer sleep to reduce CPU
        if ctx.is_streaming {
            // Streaming: use max_sleep_duration (suitable for WebSocket/SSE)
            return self.config.max_sleep_duration;
        }

        // For upload operations, use middle value
        if ctx.is_upload {
            let mid_ms = (base_sleep_ms + self.config.max_sleep_duration.as_millis() as u64) / 2;
            return Duration::from_millis(mid_ms);
        }

        // Calculate based on expected timeout - sleep should be small fraction
        if ctx.expected_body_size.is_some() {
            let read_timeout = self.calculate_read_timeout(ctx);
            // Sleep should be fraction of expected timeout, clamped to min/max
            let fraction = self.config.sleep_timeout_fraction.max(1);
            let sleep_ms = (read_timeout.as_millis() as u64 / fraction)
                .clamp(base_sleep_ms, self.config.max_sleep_duration.as_millis() as u64);
            return Duration::from_millis(sleep_ms);
        }

        Duration::from_millis(base_sleep_ms)
    }
}

impl Default for TimeoutCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TimeoutConfig::default();
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert_eq!(config.read_timeout_per_kb, Duration::from_millis(10));
        assert_eq!(config.write_timeout_per_kb, Duration::from_millis(5));
        assert_eq!(config.min_read_timeout, Duration::from_millis(100));
        assert_eq!(config.max_read_timeout, Duration::from_secs(60));
        assert_eq!(config.max_total_timeout, Duration::from_secs(300));
        assert_eq!(config.ttfb_timeout, Duration::from_secs(5));
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_size_based_calculation() {
        let calc = TimeoutCalculator::new();

        // 1 KB -> ~10ms, clamped to min 100ms
        let ctx = TimeoutContext::with_size(1024);
        let timeout = calc.calculate_read_timeout(&ctx);
        assert_eq!(timeout, Duration::from_millis(100));

        // 10 KB -> sqrt(10) * 10ms = 32ms, clamped to min 100ms
        let ctx = TimeoutContext::with_size(10 * 1024);
        let timeout = calc.calculate_read_timeout(&ctx);
        assert_eq!(timeout, Duration::from_millis(100));

        // 100 KB -> sqrt(100) * 10ms = 100ms
        let ctx = TimeoutContext::with_size(100 * 1024);
        let timeout = calc.calculate_read_timeout(&ctx);
        assert_eq!(timeout, Duration::from_millis(100));

        // 1 MB -> sqrt(1024) * 10ms = 320ms
        let ctx = TimeoutContext::with_size(1024 * 1024);
        let timeout = calc.calculate_read_timeout(&ctx);
        assert_eq!(timeout, Duration::from_millis(320));

        // 10 MB -> sqrt(10240) * 10ms = 1.01s
        let ctx = TimeoutContext::with_size(10 * 1024 * 1024);
        let timeout = calc.calculate_read_timeout(&ctx);
        assert_eq!(timeout, Duration::from_millis(1011));

        // 100 MB -> sqrt(102400) * 10ms = 3.2s
        let ctx = TimeoutContext::with_size(100 * 1024 * 1024);
        let timeout = calc.calculate_read_timeout(&ctx);
        assert_eq!(timeout, Duration::from_millis(3200));
    }

    #[test]
    fn test_bounds_clamping() {
        let calc = TimeoutCalculator::new();

        // Tiny body gets min timeout
        let ctx = TimeoutContext::with_size(10);
        let timeout = calc.calculate_read_timeout(&ctx);
        assert_eq!(timeout, Duration::from_millis(100));

        // Huge body (36GB+) gets max timeout (60s)
        // sqrt(36M KB) * 10ms = 6000 * 10ms = 60s, clamped to max
        let ctx = TimeoutContext::with_size(36 * 1024 * 1024 * 1024);
        let timeout = calc.calculate_read_timeout(&ctx);
        assert_eq!(timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_zero_size() {
        let calc = TimeoutCalculator::new();

        let ctx = TimeoutContext::default();
        let timeout = calc.calculate_read_timeout(&ctx);
        assert_eq!(timeout, Duration::from_millis(100));
    }

    #[test]
    fn test_write_timeout() {
        let calc = TimeoutCalculator::new();

        // 1 MB write: sqrt(1024) * 5ms = 160ms
        let ctx = TimeoutContext::with_size(1024 * 1024);
        let timeout = calc.calculate_write_timeout(&ctx);
        assert_eq!(timeout, Duration::from_millis(160));

        // Upload gets 1.5x factor: 160ms * 1.5 = 240ms
        let ctx = TimeoutContext::with_size(1024 * 1024).upload();
        let timeout = calc.calculate_write_timeout(&ctx);
        assert_eq!(timeout, Duration::from_millis(240));
    }

    #[test]
    fn test_total_timeout() {
        let calc = TimeoutCalculator::new();

        // 1 MB: 320ms read, 3 retries = 320ms * 4 = 1.28s
        let ctx = TimeoutContext::with_size(1024 * 1024);
        let timeout = calc.calculate_total_timeout(&ctx);
        assert_eq!(timeout, Duration::from_millis(1280));

        // Huge body (75GB+) gets clamped to max_read_timeout (60s)
        // sqrt(75M KB) * 10ms = 8660 * 10ms = 86.6s per read, clamped to 60s
        // With 4 attempts: 60s * 4 = 240s
        let ctx = TimeoutContext::with_size(75 * 1024 * 1024 * 1024);
        let timeout = calc.calculate_total_timeout(&ctx);
        assert_eq!(timeout, Duration::from_secs(240));
    }

    #[test]
    fn test_custom_config() {
        let config = TimeoutConfig {
            min_read_timeout: Duration::from_millis(50),
            max_read_timeout: Duration::from_secs(30),
            ..TimeoutConfig::default()
        };
        let calc = TimeoutCalculator::with_config(config);

        // Small body uses custom min
        let ctx = TimeoutContext::with_size(100);
        let timeout = calc.calculate_read_timeout(&ctx);
        assert_eq!(timeout, Duration::from_millis(50));

        // Huge body (9GB+) uses custom max (30s)
        // sqrt(9M KB) * 10ms = 3000 * 10ms = 30s, clamped to max
        let ctx = TimeoutContext::with_size(9 * 1024 * 1024 * 1024);
        let timeout = calc.calculate_read_timeout(&ctx);
        assert_eq!(timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_context_builder() {
        let ctx = TimeoutContext::with_endpoint("api.example.com")
            .with_body_size(1024)
            .upload()
            .streaming();

        assert_eq!(ctx.endpoint, Some("api.example.com".to_string()));
        assert_eq!(ctx.expected_body_size, Some(1024));
        assert!(ctx.is_upload);
        assert!(ctx.is_streaming);
    }
}
