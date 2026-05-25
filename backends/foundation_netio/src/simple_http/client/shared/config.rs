//! HTTP client configuration — pure data, no native dependencies.

use std::collections::BTreeMap;
use std::time::Duration;

use crate::simple_http::shared::timeout::{TimeoutCalculator, TimeoutConfig, TimeoutContext};
use crate::simple_http::shared::{SimpleHeader, SimpleHeaders, SimpleHttpBody};

use super::proxy;

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
    pub preserve_auth_on_redirect: bool,
    pub preserve_cookies_on_redirect: bool,
    pub follow_other_redirects_response: bool,
    pub headers_to_pass_on_redirect: Option<Vec<SimpleHeader>>,
    pub headers_to_add: Option<SimpleHeaders>,
}

impl ClientConfig {
    #[must_use]
    pub fn get_op_timeout(&self) -> (Duration, Duration, Duration) {
        let config = self.timeout_calculator.config();
        (config.connect_timeout, config.min_read_timeout, config.min_read_timeout)
    }

    #[must_use]
    pub fn into_simple_http_body(&self) -> SimpleHttpBody {
        SimpleHttpBody::new(
            self.max_body_size,
            self.full_body_threshold,
            self.batch_size,
            self.max_retries,
        )
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
        self.follow_other_redirects_response = follow;
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
    pub fn with_preserve_auth_on_redirect(mut self, preserve: bool) -> Self {
        self.preserve_auth_on_redirect = preserve;
        self
    }

    #[must_use]
    pub fn with_preserve_cookies_on_redirect(mut self, preserve: bool) -> Self {
        self.preserve_cookies_on_redirect = preserve;
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
            preserve_auth_on_redirect: false,
            preserve_cookies_on_redirect: false,
            follow_other_redirects_response: true,
            headers_to_pass_on_redirect: None,
            headers_to_add: None,
        }
    }
}
