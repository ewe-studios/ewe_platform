/// WHY: HTTP client needs request/response interception for cross-cutting concerns
/// like logging, timing, retry, header injection without modifying core client logic.
///
/// WHAT: Middleware trait defining handle_request/handle_response hooks, and
/// MiddlewareChain implementing onion model execution (forward for requests, reverse for responses).
///
/// HOW: Middleware trait is Send + Sync for thread safety. MiddlewareChain stores Arc<dyn Middleware>
/// for shared ownership. process_request iterates forward, process_response iterates reverse.
///
/// # Panics
///
/// Never panics.
use crate::wire::simple_http::client::PreparedRequest;
use crate::wire::simple_http::{
    HttpClientError, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleResponse,
};
use std::sync::Arc;
use tracing::{debug, info};

/// Middleware trait for request/response interception.
///
/// WHY: Enables custom processing of requests/responses without modifying core client.
///
/// WHAT: Defines two hooks: handle_request (before sending) and handle_response (after receiving).
/// Implementations can modify request/response or return errors to abort/fail.
///
/// HOW: Middleware stored in MiddlewareChain, called in order for requests, reverse for responses.
/// Must be Send + Sync for thread safety across executors.
///
/// # Examples
///
/// ```
/// use foundation_core::wire::simple_http::client::{Middleware, PreparedRequest};
/// use foundation_core::wire::simple_http::{HttpClientError, SimpleResponse};
///
/// struct HeaderMiddleware;
///
/// impl Middleware for HeaderMiddleware {
///     fn handle_request(&self, request: &mut PreparedRequest) -> Result<(), HttpClientError> {
///         request.headers.insert("X-Custom".to_string(), "value".to_string());
///         Ok(())
///     }
///
///     fn handle_response(
///         &self,
///         _request: &PreparedRequest,
///         _response: &mut SimpleResponse<SendSafeBody>,
///     ) -> Result<(), HttpClientError> {
///         Ok(())
///     }
/// }
/// ```
pub trait Middleware: Send + Sync {
    /// Called before request is sent.
    ///
    /// WHY: Allows modifying request or aborting based on custom logic.
    ///
    /// WHAT: Receives mutable request, can modify headers/body/url. Return Err to abort.
    ///
    /// HOW: Called by MiddlewareChain::process_request in forward order.
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` to abort the request.
    ///
    /// # Panics
    ///
    /// Should not panic. Implementations should return errors instead.
    fn handle_request(&self, request: &mut PreparedRequest) -> Result<(), HttpClientError>;

    /// Called after response is received.
    ///
    /// WHY: Allows modifying response or converting success to error based on custom logic.
    ///
    /// WHAT: Receives immutable request and mutable response. Return Err to fail the response.
    ///
    /// HOW: Called by MiddlewareChain::process_response in reverse order (onion model).
    ///
    /// # Errors
    ///
    /// Returns `HttpClientError` to convert successful response into error.
    ///
    /// # Panics
    ///
    /// Should not panic. Implementations should return errors instead.
    fn handle_response(
        &self,
        request: &PreparedRequest,
        response: &mut SimpleResponse<SendSafeBody>,
    ) -> Result<(), HttpClientError>;

    /// Middleware name for debugging.
    ///
    /// WHY: Useful for logging and per-request middleware skipping.
    ///
    /// WHAT: Returns static string identifying middleware type.
    ///
    /// HOW: Default implementation uses type_name.
    ///
    /// # Panics
    ///
    /// Never panics.
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

/// Chain of middleware executed in onion model pattern.
///
/// WHY: Manages middleware execution order. Requests flow forward (first to last),
/// responses flow backward (last to first) creating symmetric onion layers.
///
/// WHAT: Stores ordered list of middleware as Arc for shared ownership.
/// Provides add() for registration and process_request/process_response for execution.
///
/// HOW: Stores Vec<Arc<dyn Middleware>>. process_request iterates forward,
/// process_response iterates with .iter().rev() for reverse order.
///
/// # Examples
///
/// ```
/// use foundation_core::wire::simple_http::client::{MiddlewareChain, PreparedRequest};
///
/// let mut chain = MiddlewareChain::new();
/// // chain.add(LoggingMiddleware::new());
/// // chain.add(TimingMiddleware::new());
/// ```
pub struct MiddlewareChain {
    middlewares: Vec<Arc<dyn Middleware>>,
}

impl MiddlewareChain {
    /// Creates a new empty middleware chain.
    ///
    /// WHY: Initialize storage for middleware registration.
    ///
    /// WHAT: Returns empty MiddlewareChain with no registered middleware.
    ///
    /// HOW: Creates empty Vec.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
        }
    }

    /// Adds middleware to the chain.
    ///
    /// WHY: Register middleware for execution during requests/responses.
    ///
    /// WHAT: Appends middleware to end of chain. Later middleware execute later for requests,
    /// earlier for responses (onion model).
    ///
    /// HOW: Wraps middleware in Arc, pushes to Vec.
    ///
    /// # Panics
    ///
    /// Never panics.
    pub fn add(&mut self, middleware: impl Middleware + 'static) {
        self.middlewares.push(Arc::new(middleware));
    }

    /// Processes request through middleware chain.
    ///
    /// WHY: Execute all middleware handle_request hooks before sending.
    ///
    /// WHAT: Iterates middleware in forward order (first to last), calling handle_request.
    /// Stops on first error.
    ///
    /// HOW: Iterates &self.middlewares, calls mw.handle_request(request)?, propagates errors.
    ///
    /// # Errors
    ///
    /// Returns first `HttpClientError` returned by any middleware, aborting the chain.
    ///
    /// # Panics
    ///
    /// Never panics.
    pub fn process_request(&self, request: &mut PreparedRequest) -> Result<(), HttpClientError> {
        for mw in &self.middlewares {
            mw.handle_request(request)?;
        }
        Ok(())
    }

    /// Processes response through middleware chain.
    ///
    /// WHY: Execute all middleware handle_response hooks after receiving response.
    ///
    /// WHAT: Iterates middleware in reverse order (last to first), calling handle_response.
    /// Stops on first error. This creates onion model symmetry.
    ///
    /// HOW: Iterates self.middlewares.iter().rev(), calls mw.handle_response(request, response)?,
    /// propagates errors.
    ///
    /// # Errors
    ///
    /// Returns first `HttpClientError` returned by any middleware, aborting the chain.
    ///
    /// # Panics
    ///
    /// Never panics.
    pub fn process_response(
        &self,
        request: &PreparedRequest,
        response: &mut SimpleResponse<SendSafeBody>,
    ) -> Result<(), HttpClientError> {
        // Reverse order for responses (onion model)
        for mw in self.middlewares.iter().rev() {
            mw.handle_response(request, response)?;
        }
        Ok(())
    }
}

impl Default for MiddlewareChain {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for MiddlewareChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MiddlewareChain")
            .field("count", &self.middlewares.len())
            .finish()
    }
}

// ============================================================================
// Built-in Middleware Implementations
// ============================================================================

/// Logging middleware for debugging HTTP requests/responses.
///
/// WHY: Developers need visibility into HTTP traffic for debugging without
/// adding logging code throughout the client.
///
/// WHAT: Logs request and response details using tracing at info and debug levels.
/// Does not modify requests or responses.
///
/// HOW: Implements Middleware trait, logging method/URL/headers at info level,
/// and full header details at debug level using tracing infrastructure.
///
/// # Examples
///
/// ```
/// use foundation_core::wire::simple_http::client::{LoggingMiddleware, MiddlewareChain};
///
/// let mut chain = MiddlewareChain::new();
/// chain.add(LoggingMiddleware::new());
/// ```
pub struct LoggingMiddleware;

impl LoggingMiddleware {
    /// Creates new LoggingMiddleware.
    ///
    /// WHY: Initialize logging middleware for request/response logging.
    ///
    /// WHAT: Returns LoggingMiddleware instance.
    ///
    /// HOW: Simple struct construction.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for LoggingMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

impl Middleware for LoggingMiddleware {
    fn handle_request(&self, request: &mut PreparedRequest) -> Result<(), HttpClientError> {
        let headers_count = request.headers.len();
        info!(
            method = %request.method,
            url = %request.url,
            headers_count = headers_count,
            "HTTP request"
        );

        debug!(
            method = %request.method,
            url = %request.url,
            headers = ?request.headers,
            "HTTP request headers"
        );

        Ok(())
    }

    fn handle_response(
        &self,
        request: &PreparedRequest,
        response: &mut SimpleResponse<SendSafeBody>,
    ) -> Result<(), HttpClientError> {
        let headers_count = response.get_headers_ref().len();
        let status = response.get_status();
        info!(
            status = %status,
            url = %request.url,
            headers_count = headers_count,
            "HTTP response"
        );

        debug!(
            status = %status,
            url = %request.url,
            headers = ?response.get_headers_ref(),
            "HTTP response headers"
        );

        Ok(())
    }
}

/// Timing middleware for measuring request duration.
///
/// WHY: Performance monitoring requires measuring how long requests take
/// without adding timing code throughout the client.
///
/// WHAT: Records start time in handle_request, calculates and logs duration in
/// handle_response. Stores Instant in request extensions.
///
/// HOW: Uses std::time::Instant for timing. Stores start time with type-safe
/// Extensions in request. Logs duration using tracing::info.
///
/// # Examples
///
/// ```
/// use foundation_core::wire::simple_http::client::{TimingMiddleware, MiddlewareChain};
///
/// let mut chain = MiddlewareChain::new();
/// chain.add(TimingMiddleware::new());
/// ```
pub struct TimingMiddleware;

impl TimingMiddleware {
    /// Creates new TimingMiddleware.
    ///
    /// WHY: Initialize timing middleware for request duration measurement.
    ///
    /// WHAT: Returns TimingMiddleware instance.
    ///
    /// HOW: Simple struct construction.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for TimingMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

impl Middleware for TimingMiddleware {
    fn handle_request(&self, request: &mut PreparedRequest) -> Result<(), HttpClientError> {
        // Store start time in extensions
        request.extensions.insert(std::time::Instant::now());
        Ok(())
    }

    fn handle_response(
        &self,
        request: &PreparedRequest,
        _response: &mut SimpleResponse<SendSafeBody>,
    ) -> Result<(), HttpClientError> {
        // Calculate and log duration if start time exists
        if let Some(start) = request.extensions.get::<std::time::Instant>() {
            let duration = start.elapsed();
            info!(
                url = %request.url,
                duration_ms = duration.as_millis(),
                "HTTP request completed"
            );
        }
        Ok(())
    }
}

/// Header middleware for adding default headers to requests.
///
/// WHY: Many requests need common headers (User-Agent, Accept, etc.) without
/// manually setting them each time.
///
/// WHAT: Adds configured headers to requests only if they don't already exist.
/// Does not overwrite existing headers.
///
/// HOW: Stores headers in BTreeMap, checks if header exists before inserting.
///
/// # Examples
///
/// ```
/// use foundation_core::wire::simple_http::client::{HeaderMiddleware, MiddlewareChain};
/// use foundation_core::wire::simple_http::SimpleHeader;
///
/// let mut chain = MiddlewareChain::new();
/// chain.add(
///     HeaderMiddleware::new()
///         .with_header(SimpleHeader::USER_AGENT, "MyApp/1.0".to_string())
///         .with_header(SimpleHeader::ACCEPT, "application/json".to_string())
/// );
/// ```
pub struct HeaderMiddleware {
    headers: SimpleHeaders,
}

impl HeaderMiddleware {
    /// Creates new HeaderMiddleware with no default headers.
    ///
    /// WHY: Initialize middleware before adding headers via builder pattern.
    ///
    /// WHAT: Returns empty HeaderMiddleware.
    ///
    /// HOW: Creates empty BTreeMap for headers.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn new() -> Self {
        Self {
            headers: SimpleHeaders::new(),
        }
    }

    /// Adds a default header.
    ///
    /// WHY: Builder pattern for configuring multiple default headers.
    ///
    /// WHAT: Adds header to be applied to requests if not already present.
    ///
    /// HOW: Inserts into internal headers map, returns self for chaining.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn with_header(mut self, name: SimpleHeader, value: String) -> Self {
        self.headers.insert(name, vec![value]);
        self
    }
}

impl Default for HeaderMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

impl Middleware for HeaderMiddleware {
    fn handle_request(&self, request: &mut PreparedRequest) -> Result<(), HttpClientError> {
        // Add default headers only if not already present
        for (name, values) in &self.headers {
            if !request.headers.contains_key(name) {
                request.headers.insert(name.clone(), values.clone());
            }
        }
        Ok(())
    }

    fn handle_response(
        &self,
        _request: &PreparedRequest,
        _response: &mut SimpleResponse<SendSafeBody>,
    ) -> Result<(), HttpClientError> {
        Ok(())
    }
}

// ============================================================================
// Retry Middleware and Supporting Types
// ============================================================================

/// Backoff strategy for retry delays.
///
/// WHY: Different retry scenarios need different delay patterns. Constant for predictable
/// delays, Linear for gradual increase, Exponential for rapid backoff.
///
/// WHAT: Enum defining three strategies with their parameters. Each calculates
/// delay based on attempt number.
///
/// HOW: next_delay() takes attempt number and returns Duration based on formula
/// specific to each variant.
///
/// # Examples
///
/// ```
/// use foundation_core::wire::simple_http::client::BackoffStrategy;
/// use std::time::Duration;
///
/// let constant = BackoffStrategy::Constant {
///     delay: Duration::from_millis(100),
/// };
/// assert_eq!(constant.next_delay(1), Duration::from_millis(100));
///
/// let linear = BackoffStrategy::Linear {
///     base: Duration::from_millis(100),
///     increment: Duration::from_millis(50),
/// };
/// assert_eq!(linear.next_delay(1), Duration::from_millis(150)); // 100 + 50*1
///
/// let exponential = BackoffStrategy::Exponential {
///     base: Duration::from_millis(100),
///     multiplier: 2.0,
/// };
/// assert_eq!(exponential.next_delay(1), Duration::from_millis(200)); // 100 * 2^1
/// ```
#[derive(Debug, Clone)]
pub enum BackoffStrategy {
    /// Constant delay for all retries.
    Constant {
        /// Fixed delay between retries.
        delay: std::time::Duration,
    },
    /// Linear backoff: base + (increment * attempt).
    Linear {
        /// Initial delay.
        base: std::time::Duration,
        /// Amount to add per attempt.
        increment: std::time::Duration,
    },
    /// Exponential backoff: base * multiplier^attempt.
    Exponential {
        /// Initial delay.
        base: std::time::Duration,
        /// Multiplier for exponential growth.
        multiplier: f64,
    },
}

impl BackoffStrategy {
    /// Calculates delay for given attempt number.
    ///
    /// WHY: Retry logic needs to know how long to wait before next attempt.
    ///
    /// WHAT: Computes Duration based on strategy type and attempt number.
    ///
    /// HOW: Match on strategy variant, apply appropriate formula.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn next_delay(&self, attempt: u32) -> std::time::Duration {
        match self {
            Self::Constant { delay } => *delay,
            Self::Linear { base, increment } => *base + (*increment * attempt),
            Self::Exponential { base, multiplier } => {
                let factor = multiplier.powi(attempt as i32);
                base.mul_f64(factor)
            }
        }
    }
}

/// Tracks retry state for a request.
///
/// WHY: RetryMiddleware needs to track how many times a request has been retried
/// to enforce max_retries limit.
///
/// WHAT: Stores current attempt number. Stored in request Extensions.
///
/// HOW: Incremented in handle_response when retry is needed. Checked against max_retries.
///
/// # Examples
///
/// ```
/// use foundation_core::wire::simple_http::client::RetryState;
///
/// let state = RetryState::new(3);
/// assert_eq!(state.attempt, 0);
/// ```
#[derive(Debug, Clone)]
pub struct RetryState {
    /// Current retry attempt number (0-indexed).
    pub attempt: u32,
}

impl RetryState {
    /// Creates new RetryState with attempt counter at 0.
    ///
    /// WHY: Initialize retry tracking for a new request.
    ///
    /// WHAT: Returns RetryState with attempt = 0.
    ///
    /// HOW: Simple struct construction.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn new(_max_retries: u32) -> Self {
        Self { attempt: 0 }
    }
}

/// Retry middleware for automatic request retries on specific status codes.
///
/// WHY: Network requests can fail transiently (rate limits, temporary server errors).
/// Automatic retry improves reliability without manual intervention.
///
/// WHAT: Middleware that retries requests when response status matches configured codes.
/// Uses configurable backoff strategy for delays between attempts.
///
/// HOW: Stores RetryState in request extensions. On matching status code, increments
/// attempt and returns error if max_retries not exceeded. Error signals retry needed.
///
/// # Examples
///
/// ```
/// use foundation_core::wire::simple_http::client::RetryMiddleware;
///
/// let retry = RetryMiddleware::new(3, vec![429, 502, 503, 504]);
/// ```
pub struct RetryMiddleware {
    max_retries: u32,
    retry_status_codes: Vec<u16>,
    backoff: BackoffStrategy,
}

impl RetryMiddleware {
    /// Creates new RetryMiddleware with exponential backoff.
    ///
    /// WHY: Configure retry behavior with max attempts and status codes.
    ///
    /// WHAT: Returns RetryMiddleware with default exponential backoff (100ms base, 2x multiplier).
    ///
    /// HOW: Stores parameters, initializes default BackoffStrategy.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn new(max_retries: u32, retry_status_codes: Vec<u16>) -> Self {
        Self {
            max_retries,
            retry_status_codes,
            backoff: BackoffStrategy::Exponential {
                base: std::time::Duration::from_millis(100),
                multiplier: 2.0,
            },
        }
    }

    /// Sets custom backoff strategy.
    ///
    /// WHY: Different use cases need different retry delay patterns.
    ///
    /// WHAT: Builder method to override default exponential backoff.
    ///
    /// HOW: Replaces backoff field, returns self.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn with_backoff(mut self, backoff: BackoffStrategy) -> Self {
        self.backoff = backoff;
        self
    }
}

impl Middleware for RetryMiddleware {
    fn handle_request(&self, request: &mut PreparedRequest) -> Result<(), HttpClientError> {
        // Store retry state in extensions
        request.extensions.insert(RetryState::new(self.max_retries));
        Ok(())
    }

    fn handle_response(
        &self,
        request: &PreparedRequest,
        response: &mut SimpleResponse<SendSafeBody>,
    ) -> Result<(), HttpClientError> {
        let status = response.get_status().into_usize() as u16;

        // Check if status code matches retry conditions
        if self.retry_status_codes.contains(&status) {
            if let Some(state) = request.extensions.get::<RetryState>() {
                if state.attempt < self.max_retries {
                    // TODO: Need to return retry error - but HttpClientError doesn't have RetryNeeded variant yet
                    // For now, just track the state
                    // This will be completed when HttpClientError is extended
                }
            }
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "RetryMiddleware"
    }
}
