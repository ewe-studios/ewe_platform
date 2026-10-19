//! HTTP client configuration — pure data, no native dependencies.

use std::collections::BTreeMap;
use std::time::Duration;

use crate::shared::http::timeout::{TimeoutCalculator, TimeoutConfig, TimeoutContext};
use crate::shared::http::framing::FrameLimits;
use crate::shared::http::{SimpleHeader, SimpleHeaders, SimpleHttpBody};

use super::proxy;

/// Redirect-handling policy.
///
/// Grouped into its own struct so the boolean toggles that govern redirect
/// behaviour live in one cohesive place (and keep `ClientConfig`'s boolean
/// field count manageable).
#[derive(Debug, Clone)]
pub struct RedirectConfig {
    /// Preserve the `Authorization` header when following a cross-host redirect.
    pub preserve_auth_on_redirect: bool,
    /// Preserve the `Cookie` header when following a cross-host redirect.
    pub preserve_cookies_on_redirect: bool,
    /// Follow `3xx` responses other than the canonical redirect statuses.
    pub follow_other_redirects_response: bool,
}

impl Default for RedirectConfig {
    fn default() -> Self {
        Self {
            preserve_auth_on_redirect: false,
            preserve_cookies_on_redirect: false,
            follow_other_redirects_response: true,
        }
    }
}

/// Configuration for HTTP client.
///
/// Centralizes all client configuration in one place. Makes it easy to
/// share configuration across requests or customize per-instance.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub expect_continue_read_timeout: Duration,
    pub inline_processing_timeout: Duration,
    pub timeout_calculator: TimeoutCalculator,
    pub max_redirects: u8,
    pub default_headers: SimpleHeaders,
    pub proxy: Option<proxy::ProxyConfig>,
    pub proxy_from_env: bool,
    pub max_body_size: Option<u64>,
    pub full_body_threshold: u64,
    pub batch_size: usize,
    pub max_retries: usize,
    /// Caps on the frames a parser must buffer whole: the response header
    /// block, a chunk-size line, one line of a line-delimited body, one
    /// server-sent event. Body data is never capped by these; it streams in
    /// `batch_size` pieces.
    pub frame_limits: FrameLimits,
    /// Redirect-following policy (auth/cookie preservation, etc.).
    pub redirect: RedirectConfig,
    /// When `true`, requests carrying a body negotiate with an
    /// `Expect: 100-continue` header and wait for the server's interim
    /// `100 Continue` response before streaming the body. When `false`, the
    /// header is never sent and the body is written immediately. Bodyless
    /// requests (e.g. GET) never use the handshake regardless of this flag.
    pub expect_continue_enabled: bool,
    pub headers_to_pass_on_redirect: Option<Vec<SimpleHeader>>,
    pub headers_to_add: Option<SimpleHeaders>,
    /// Connect via a Unix domain socket instead of TCP (Docker daemon,
    /// BuildKitd). When set, connection routing bypasses DNS/proxy and dials
    /// this socket path. `None` (default) uses the normal TCP/TLS path.
    #[cfg(unix)]
    pub unix_socket: Option<std::path::PathBuf>,
}

impl ClientConfig {
    #[must_use]
    pub fn get_op_timeout(&self) -> (Duration, Duration, Duration) {
        let config = self.timeout_calculator.config();
        (
            config.connect_timeout,
            config.min_read_timeout,
            config.min_read_timeout,
        )
    }

    #[must_use]
    pub fn into_simple_http_body(&self) -> SimpleHttpBody {
        SimpleHttpBody::new(
            self.max_body_size,
            self.full_body_threshold,
            self.batch_size,
            self.max_retries,
        )
        .with_frame_limits(self.frame_limits)
    }

    #[must_use]
    pub fn calculate_read_timeout(
        &self,
        expected_body_size: Option<usize>,
        is_upload: bool,
    ) -> Duration {
        let mut ctx = TimeoutContext::default();
        if let Some(size) = expected_body_size {
            ctx.expected_body_size = Some(size);
        }
        ctx.is_upload = is_upload;
        self.timeout_calculator.calculate_read_timeout(&ctx)
    }

    #[must_use]
    pub fn get_expect_continue_read_timeout(&self) -> Duration {
        self.expect_continue_read_timeout
    }

    /// How long a non-blocking request waits for the response status line and
    /// headers, with no bytes arriving, before failing with `Timeout`.
    ///
    /// WHY: a blocking socket enforced this in the kernel (`SO_RCVTIMEO`). A
    /// parked task enforces it as a deadline instead. It is the configured
    /// time-to-first-byte budget.
    #[must_use]
    pub fn response_head_timeout(&self) -> Duration {
        self.timeout_calculator.config().ttfb_timeout
    }

    /// How long a non-blocking exchange may go with no progress while it
    /// streams: the server not accepting request bytes, or not sending response
    /// body bytes.
    ///
    /// WHY: a parked task holds no worker, so a long-idle stream (a slow
    /// server-streaming RPC) costs nothing while it waits. The bound only has to
    /// catch a dead peer, so it uses the generous maximum read timeout.
    #[must_use]
    pub fn io_idle_timeout(&self) -> Duration {
        self.timeout_calculator.config().max_read_timeout
    }

    /// The read timeout a connection carries when its response body is handed
    /// to a synchronous reader.
    ///
    /// WHY: the body iterators (`body_reader::collect_string`, the
    /// `RequestIntro::Success` stream) rely on blocking reads. Their
    /// `WouldBlock`/`TimedOut` retry budget assumes each retry waited a read
    /// timeout. The blocking client always left the expect-continue read timeout
    /// on the socket at this point, so the handoff restores exactly that.
    #[must_use]
    pub fn blocking_body_read_timeout(&self) -> Duration {
        self.expect_continue_read_timeout
    }

    #[must_use]
    pub fn calculate_write_timeout(&self, body_size: Option<usize>) -> Duration {
        let ctx = TimeoutContext::with_size(body_size.unwrap_or(0));
        self.timeout_calculator.calculate_write_timeout(&ctx)
    }

    #[must_use]
    pub fn timeout_config(&self) -> &TimeoutConfig {
        self.timeout_calculator.config()
    }

    #[must_use]
    pub fn with_follow_other_redirects_response(mut self, follow: bool) -> Self {
        self.redirect.follow_other_redirects_response = follow;
        self
    }

    #[must_use]
    pub fn with_max_body_size(mut self, max_body_size: Option<u64>) -> Self {
        self.max_body_size = max_body_size;
        self
    }

    #[must_use]
    pub fn with_headers_to_pass_on_redirect(mut self, headers: Option<Vec<SimpleHeader>>) -> Self {
        self.headers_to_pass_on_redirect = headers;
        self
    }

    #[must_use]
    pub fn with_headers_to_add(mut self, headers: Option<SimpleHeaders>) -> Self {
        self.headers_to_add = headers;
        self
    }

    #[must_use]
    pub fn set_expect_continue_read_timeout(mut self, timeout: Duration) -> Self {
        self.expect_continue_read_timeout = timeout;
        self
    }

    #[must_use]
    pub fn with_full_body_threshold(mut self, threshold: u64) -> Self {
        self.full_body_threshold = threshold;
        self
    }

    #[must_use]
    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size;
        self
    }

    /// Replace all frame caps at once.
    #[must_use]
    pub fn with_frame_limits(mut self, limits: FrameLimits) -> Self {
        self.frame_limits = limits;
        self
    }

    /// Cap the response header block (and a chunked body's trailer section).
    #[must_use]
    pub fn with_max_header_block(mut self, bytes: usize) -> Self {
        self.frame_limits.max_header_block = bytes;
        self
    }

    /// Cap a chunked body's chunk-size line (size octets and extensions).
    #[must_use]
    pub fn with_max_chunk_header_line(mut self, bytes: usize) -> Self {
        self.frame_limits.max_chunk_header_line = bytes;
        self
    }

    /// Cap one line of a line-delimited body.
    #[must_use]
    pub fn with_max_line(mut self, bytes: usize) -> Self {
        self.frame_limits.max_line = bytes;
        self
    }

    /// Cap one server-sent event.
    #[must_use]
    pub fn with_max_sse_event(mut self, bytes: usize) -> Self {
        self.frame_limits.max_sse_event = bytes;
        self
    }

    #[must_use]
    pub fn with_max_retries(mut self, max_retries: usize) -> Self {
        self.max_retries = max_retries;
        self
    }

    #[must_use]
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        let mut config = *self.timeout_calculator.config();
        config.connect_timeout = timeout;
        self.timeout_calculator = TimeoutCalculator::with_config(config);
        self
    }

    #[must_use]
    pub fn with_inline_processing_timeout(mut self, timeout: Duration) -> Self {
        self.inline_processing_timeout = timeout;
        self
    }

    #[must_use]
    pub fn with_read_timeout(mut self, timeout: Duration) -> Self {
        let mut config = *self.timeout_calculator.config();
        config.min_read_timeout = timeout;
        self.timeout_calculator = TimeoutCalculator::with_config(config);
        self
    }

    #[must_use]
    pub fn with_write_timeout(mut self, timeout: Duration) -> Self {
        let mut config = *self.timeout_calculator.config();
        config.write_timeout_per_kb = timeout;
        self.timeout_calculator = TimeoutCalculator::with_config(config);
        self
    }

    #[must_use]
    pub fn with_expect_continue(mut self, enabled: bool) -> Self {
        self.expect_continue_enabled = enabled;
        self
    }

    #[must_use]
    pub fn with_preserve_auth_on_redirect(mut self, preserve: bool) -> Self {
        self.redirect.preserve_auth_on_redirect = preserve;
        self
    }

    #[must_use]
    pub fn with_preserve_cookies_on_redirect(mut self, preserve: bool) -> Self {
        self.redirect.preserve_cookies_on_redirect = preserve;
        self
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            expect_continue_read_timeout: Duration::from_secs(3),
            inline_processing_timeout: Duration::from_millis(10),
            timeout_calculator: TimeoutCalculator::new(),
            default_headers: BTreeMap::default(),
            max_redirects: 5,
            proxy: None,
            proxy_from_env: false,
            max_body_size: None,
            full_body_threshold: 512 * 1024,
            batch_size: 8192,
            max_retries: 5,
            frame_limits: FrameLimits::default(),
            redirect: RedirectConfig::default(),
            expect_continue_enabled: true,
            headers_to_pass_on_redirect: None,
            headers_to_add: None,
            #[cfg(unix)]
            unix_socket: None,
        }
    }
}
