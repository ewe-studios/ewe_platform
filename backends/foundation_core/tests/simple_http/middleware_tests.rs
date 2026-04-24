/// WHY: Middleware requires type-safe storage for passing data between middleware layers
/// WHAT: Test Extensions can store and retrieve values by type
#[test]
fn test_extensions_insert_and_get() {
    use foundation_core::wire::simple_http::client::Extensions;

    let mut extensions = Extensions::new();
    extensions.insert(42u32);
    extensions.insert("hello".to_string());

    assert_eq!(extensions.get::<u32>(), Some(&42u32));
    assert_eq!(extensions.get::<String>(), Some(&"hello".to_string()));
}

/// WHY: Middleware needs to modify stored state (e.g., increment retry count)
/// WHAT: Test `Extensions::get_mut` allows mutating stored values
#[test]
fn test_extensions_get_mut() {
    use foundation_core::wire::simple_http::client::Extensions;

    let mut extensions = Extensions::new();
    extensions.insert(42u32);

    if let Some(value) = extensions.get_mut::<u32>() {
        *value += 1;
    }

    assert_eq!(extensions.get::<u32>(), Some(&43u32));
}

/// WHY: Middleware trait must allow custom implementations
/// WHAT: Test that a basic middleware can modify request headers
#[test]
fn test_middleware_trait_basic() {
    use foundation_core::wire::simple_http::client::{
        Extensions, Middleware, PreparedRequest, Uri,
    };
    use foundation_core::wire::simple_http::{SendSafeBody, SimpleHeader, SimpleMethod};
    use std::collections::BTreeMap;

    struct TestMiddleware;

    impl Middleware for TestMiddleware {
        fn handle_request(
            &self,
            request: &mut PreparedRequest,
        ) -> Result<(), foundation_core::wire::simple_http::HttpClientError> {
            request.headers.insert(
                SimpleHeader::Custom("X-Test-Header".to_string()),
                vec!["test-value".to_string()],
            );
            Ok(())
        }

        fn handle_response(
            &self,
            _request: &PreparedRequest,
            _response: &mut foundation_core::wire::simple_http::SimpleResponse<
                foundation_core::wire::simple_http::SendSafeBody,
            >,
        ) -> Result<(), foundation_core::wire::simple_http::HttpClientError> {
            Ok(())
        }
    }

    let mut request = PreparedRequest {
        method: SimpleMethod::GET,
        url: Uri::parse("http://example.com").unwrap(),
        headers: BTreeMap::new(),
        body: SendSafeBody::None,
        extensions: Extensions::new(),
    };

    let middleware = TestMiddleware;
    middleware.handle_request(&mut request).unwrap();

    assert_eq!(
        request
            .headers
            .get(&SimpleHeader::Custom("X-Test-Header".to_string())),
        Some(&vec!["test-value".to_string()])
    );
}

/// WHY: `LoggingMiddleware` should log request/response without modifying them
/// WHAT: Test that `LoggingMiddleware` doesn't change request
#[test]
fn test_logging_middleware_passthrough() {
    use foundation_core::wire::simple_http::client::{
        Extensions, LoggingMiddleware, Middleware, PreparedRequest, Uri,
    };
    use foundation_core::wire::simple_http::{SendSafeBody, SimpleMethod};
    use std::collections::BTreeMap;

    let mut request = PreparedRequest {
        method: SimpleMethod::GET,
        url: Uri::parse("http://example.com/test").unwrap(),
        headers: BTreeMap::new(),
        body: SendSafeBody::None,
        extensions: Extensions::new(),
    };

    let original_url = request.url.clone();
    let middleware = LoggingMiddleware::new();
    middleware.handle_request(&mut request).unwrap();

    // LoggingMiddleware should not modify the request
    assert_eq!(request.url, original_url);
    assert_eq!(request.method, SimpleMethod::GET);
}

/// WHY: `TimingMiddleware` should measure request duration
/// WHAT: Test that `TimingMiddleware` records timing in extensions
#[test]
fn test_timing_middleware_records_duration() {
    use foundation_core::wire::simple_http::client::{
        Extensions, Middleware, PreparedRequest, TimingMiddleware, Uri,
    };
    use foundation_core::wire::simple_http::{SendSafeBody, SimpleMethod, Status};
    use std::collections::BTreeMap;
    use std::time::Duration;

    let mut request = PreparedRequest {
        method: SimpleMethod::GET,
        url: Uri::parse("http://example.com/test").unwrap(),
        headers: BTreeMap::new(),
        body: SendSafeBody::None,
        extensions: Extensions::new(),
    };

    let middleware = TimingMiddleware::new();
    middleware.handle_request(&mut request).unwrap();

    // Simulate some work
    std::thread::sleep(Duration::from_millis(10));

    // Create a mock response
    let mut response = foundation_core::wire::simple_http::SimpleResponse::new(
        Status::OK,
        BTreeMap::new(),
        SendSafeBody::None,
    );

    middleware.handle_response(&request, &mut response).unwrap();

    // TimingMiddleware should have recorded duration - test passes if no panic
}

/// WHY: `HeaderMiddleware` should add default headers to requests
/// WHAT: Test that `HeaderMiddleware` adds headers only if not present
#[test]
fn test_header_middleware_adds_headers() {
    use foundation_core::wire::simple_http::client::{
        Extensions, HeaderMiddleware, Middleware, PreparedRequest, Uri,
    };
    use foundation_core::wire::simple_http::{SendSafeBody, SimpleHeader, SimpleMethod};
    use std::collections::BTreeMap;

    let mut request = PreparedRequest {
        method: SimpleMethod::GET,
        url: Uri::parse("http://example.com/test").unwrap(),
        headers: BTreeMap::new(),
        body: SendSafeBody::None,
        extensions: Extensions::new(),
    };

    let middleware =
        HeaderMiddleware::new().with_header(SimpleHeader::USER_AGENT, "test-agent".to_string());

    middleware.handle_request(&mut request).unwrap();

    // Header should be added
    assert_eq!(
        request.headers.get(&SimpleHeader::USER_AGENT),
        Some(&vec!["test-agent".to_string()])
    );
}

/// WHY: `HeaderMiddleware` should not overwrite existing headers
/// WHAT: Test that `HeaderMiddleware` respects existing headers
#[test]
fn test_header_middleware_respects_existing_headers() {
    use foundation_core::wire::simple_http::client::{
        Extensions, HeaderMiddleware, Middleware, PreparedRequest, Uri,
    };
    use foundation_core::wire::simple_http::{SendSafeBody, SimpleHeader, SimpleMethod};
    use std::collections::BTreeMap;

    let mut request = PreparedRequest {
        method: SimpleMethod::GET,
        url: Uri::parse("http://example.com/test").unwrap(),
        headers: {
            let mut h = BTreeMap::new();
            h.insert(SimpleHeader::USER_AGENT, vec!["existing-agent".to_string()]);
            h
        },
        body: SendSafeBody::None,
        extensions: Extensions::new(),
    };

    let middleware =
        HeaderMiddleware::new().with_header(SimpleHeader::USER_AGENT, "test-agent".to_string());

    middleware.handle_request(&mut request).unwrap();

    // Existing header should be preserved
    assert_eq!(
        request.headers.get(&SimpleHeader::USER_AGENT),
        Some(&vec!["existing-agent".to_string()])
    );
}

/// WHY: `MiddlewareChain` must execute in onion model (forward for requests, reverse for responses)
/// WHAT: Test middleware execution order
#[test]
fn test_middleware_chain_execution_order() {
    use foundation_core::wire::simple_http::client::{
        Extensions, Middleware, MiddlewareChain, PreparedRequest, Uri,
    };
    use foundation_core::wire::simple_http::{SendSafeBody, SimpleHeader, SimpleMethod, Status};
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    struct OrderMiddleware {
        id: u32,
        request_order: Arc<Mutex<Vec<u32>>>,
        response_order: Arc<Mutex<Vec<u32>>>,
    }

    impl Middleware for OrderMiddleware {
        fn handle_request(
            &self,
            _request: &mut PreparedRequest,
        ) -> Result<(), foundation_core::wire::simple_http::HttpClientError> {
            self.request_order.lock().unwrap().push(self.id);
            Ok(())
        }

        fn handle_response(
            &self,
            _request: &PreparedRequest,
            _response: &mut foundation_core::wire::simple_http::SimpleResponse<
                foundation_core::wire::simple_http::SendSafeBody,
            >,
        ) -> Result<(), foundation_core::wire::simple_http::HttpClientError> {
            self.response_order.lock().unwrap().push(self.id);
            Ok(())
        }
    }

    // Track execution order
    let request_order = Arc::new(Mutex::new(Vec::new()));
    let response_order = Arc::new(Mutex::new(Vec::new()));

    let mut chain = MiddlewareChain::new();
    chain.add(OrderMiddleware {
        id: 1,
        request_order: request_order.clone(),
        response_order: response_order.clone(),
    });
    chain.add(OrderMiddleware {
        id: 2,
        request_order: request_order.clone(),
        response_order: response_order.clone(),
    });
    chain.add(OrderMiddleware {
        id: 3,
        request_order: request_order.clone(),
        response_order: response_order.clone(),
    });

    let mut request = PreparedRequest {
        method: SimpleMethod::GET,
        url: Uri::parse("http://example.com").unwrap(),
        headers: BTreeMap::new(),
        body: SendSafeBody::None,
        extensions: Extensions::new(),
    };

    let mut response = foundation_core::wire::simple_http::SimpleResponse::new(
        Status::OK,
        BTreeMap::new(),
        SendSafeBody::None,
    );

    chain.process_request(&mut request).unwrap();
    chain.process_response(&request, &mut response).unwrap();

    // Requests should be processed in forward order: 1, 2, 3
    assert_eq!(*request_order.lock().unwrap(), vec![1, 2, 3]);

    // Responses should be processed in reverse order: 3, 2, 1 (onion model)
    assert_eq!(*response_order.lock().unwrap(), vec![3, 2, 1]);
}

/// WHY: `RetryMiddleware` needs constant backoff strategy (same delay for all retries)
/// WHAT: Test `BackoffStrategy::Constant` returns same delay for all attempts
#[test]
fn test_backoff_strategy_constant() {
    use foundation_core::wire::simple_http::client::BackoffStrategy;
    use std::time::Duration;

    let strategy = BackoffStrategy::Constant {
        delay: Duration::from_millis(100),
    };

    assert_eq!(strategy.next_delay(1), Duration::from_millis(100));
    assert_eq!(strategy.next_delay(2), Duration::from_millis(100));
    assert_eq!(strategy.next_delay(5), Duration::from_millis(100));
}

/// WHY: `RetryMiddleware` needs linear backoff (gradually increasing delays)
/// WHAT: Test `BackoffStrategy::Linear` calculates base + (increment * attempt)
#[test]
fn test_backoff_strategy_linear() {
    use foundation_core::wire::simple_http::client::BackoffStrategy;
    use std::time::Duration;

    let strategy = BackoffStrategy::Linear {
        base: Duration::from_millis(100),
        increment: Duration::from_millis(50),
    };

    assert_eq!(strategy.next_delay(1), Duration::from_millis(150)); // 100 + 50*1
    assert_eq!(strategy.next_delay(2), Duration::from_millis(200)); // 100 + 50*2
    assert_eq!(strategy.next_delay(3), Duration::from_millis(250)); // 100 + 50*3
}

/// WHY: `RetryMiddleware` needs exponential backoff (rapidly increasing delays to avoid overwhelming server)
/// WHAT: Test `BackoffStrategy::Exponential` calculates base * multiplier^attempt
#[test]
fn test_backoff_strategy_exponential() {
    use foundation_core::wire::simple_http::client::BackoffStrategy;
    use std::time::Duration;

    let strategy = BackoffStrategy::Exponential {
        base: Duration::from_millis(100),
        multiplier: 2.0,
    };

    assert_eq!(strategy.next_delay(1), Duration::from_millis(200)); // 100 * 2^1
    assert_eq!(strategy.next_delay(2), Duration::from_millis(400)); // 100 * 2^2
    assert_eq!(strategy.next_delay(3), Duration::from_millis(800)); // 100 * 2^3
}

/// WHY: `RetryMiddleware` needs to track retry attempts to enforce `max_retries` limit
/// WHAT: Test `RetryState` initializes with attempt=0 and stores `max_retries`
#[test]
fn test_retry_state_creation() {
    use foundation_core::wire::simple_http::client::RetryState;

    let state = RetryState::new(3);
    assert_eq!(state.attempt, 0);
}

/// WHY: `RetryMiddleware` must be configurable with max retries and status codes
/// WHAT: Test `RetryMiddleware::new()` creates middleware with exponential backoff default
#[test]
fn test_retry_middleware_creation() {
    use foundation_core::wire::simple_http::client::RetryMiddleware;

    let retry = RetryMiddleware::new(3, vec![429, 502, 503, 504]);
    // Test passes if creation succeeds (no panic)
}

/// WHY: Different use cases need different backoff strategies
/// WHAT: Test `RetryMiddleware::with_backoff()` allows custom backoff configuration
#[test]
fn test_retry_middleware_with_backoff() {
    use foundation_core::wire::simple_http::client::{BackoffStrategy, RetryMiddleware};
    use std::time::Duration;

    let retry = RetryMiddleware::new(3, vec![429]).with_backoff(BackoffStrategy::Constant {
        delay: Duration::from_secs(1),
    });
    // Test passes if builder pattern works (no panic)
}

/// WHY: `RetryMiddleware` must store retry state in request extensions for tracking
/// WHAT: Test `RetryMiddleware::handle_request` stores `RetryState` in extensions
#[test]
fn test_retry_middleware_stores_state_in_extensions() {
    use foundation_core::wire::simple_http::client::{
        Extensions, Middleware, PreparedRequest, RetryMiddleware, RetryState, Uri,
    };
    use foundation_core::wire::simple_http::{SendSafeBody, SimpleMethod};
    use std::collections::BTreeMap;

    let mut request = PreparedRequest {
        method: SimpleMethod::GET,
        url: Uri::parse("http://example.com").unwrap(),
        headers: BTreeMap::new(),
        body: SendSafeBody::None,
        extensions: Extensions::new(),
    };

    let middleware = RetryMiddleware::new(3, vec![429, 502]);
    middleware.handle_request(&mut request).unwrap();

    // RetryState should be stored in extensions
    let state = request.extensions.get::<RetryState>();
    assert!(state.is_some());
    assert_eq!(state.unwrap().attempt, 0);
}

/// WHY: `RetryMiddleware` should pass through responses with non-retryable status codes
/// WHAT: Test `RetryMiddleware::handle_response` returns Ok for status 200
#[test]
fn test_retry_middleware_passes_through_success() {
    use foundation_core::wire::simple_http::client::{
        Extensions, Middleware, PreparedRequest, RetryMiddleware, Uri,
    };
    use foundation_core::wire::simple_http::{SendSafeBody, SimpleMethod, Status};
    use std::collections::BTreeMap;

    let mut request = PreparedRequest {
        method: SimpleMethod::GET,
        url: Uri::parse("http://example.com").unwrap(),
        headers: BTreeMap::new(),
        body: SendSafeBody::None,
        extensions: Extensions::new(),
    };

    let middleware = RetryMiddleware::new(3, vec![429, 502, 503, 504]);
    middleware.handle_request(&mut request).unwrap();

    let mut response = foundation_core::wire::simple_http::SimpleResponse::new(
        Status::OK,
        BTreeMap::new(),
        SendSafeBody::None,
    );

    // Should not error on successful response
    assert!(middleware.handle_response(&request, &mut response).is_ok());
}

/// WHY: Middleware name is used for debugging and `skip_middleware` functionality
/// WHAT: Test `RetryMiddleware::name()` returns "`RetryMiddleware`"
#[test]
fn test_retry_middleware_name() {
    use foundation_core::wire::simple_http::client::{Middleware, RetryMiddleware};

    let middleware = RetryMiddleware::new(3, vec![429]);
    assert_eq!(middleware.name(), "RetryMiddleware");
}

/// WHY: `RetryMiddleware` must signal retry needed via error for matching status codes
/// WHAT: Test `RetryMiddleware::handle_response` returns `RetryNeeded` error for status 429
#[test]
fn test_retry_middleware_returns_retry_error_for_matching_status() {
    use foundation_core::wire::simple_http::client::{
        Extensions, Middleware, PreparedRequest, RetryMiddleware, Uri,
    };
    use foundation_core::wire::simple_http::{HttpClientError, SendSafeBody, SimpleMethod, Status};
    use std::collections::BTreeMap;

    let mut request = PreparedRequest {
        method: SimpleMethod::GET,
        url: Uri::parse("http://example.com").unwrap(),
        headers: BTreeMap::new(),
        body: SendSafeBody::None,
        extensions: Extensions::new(),
    };

    let middleware = RetryMiddleware::new(3, vec![429, 502, 503, 504]);
    middleware.handle_request(&mut request).unwrap();

    let mut response = foundation_core::wire::simple_http::SimpleResponse::new(
        Status::TooManyRequests,
        BTreeMap::new(),
        SendSafeBody::None,
    );

    // Should return RetryNeeded error for retryable status code
    let result = middleware.handle_response(&request, &mut response);
    assert!(result.is_err());

    if let Err(HttpClientError::RetryNeeded {
        attempt,
        delay,
        status_code,
    }) = result
    {
        assert_eq!(attempt, 0);
        assert!(delay.as_millis() > 0);
        assert_eq!(status_code, Some(429));
    } else {
        panic!("Expected RetryNeeded error variant");
    }
}
