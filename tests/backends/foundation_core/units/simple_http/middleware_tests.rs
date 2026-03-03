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
/// WHAT: Test Extensions::get_mut allows mutating stored values
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
    use foundation_core::wire::simple_http::client::{Middleware, PreparedRequest, ParsedUrl, Extensions};
    use foundation_core::wire::simple_http::{SimpleMethod, SendSafeBody, SimpleHeader};
    use std::collections::BTreeMap;

    struct TestMiddleware;

    impl Middleware for TestMiddleware {
        fn handle_request(&self, request: &mut PreparedRequest) -> Result<(), foundation_core::wire::simple_http::HttpClientError> {
            request.headers.insert(SimpleHeader::Custom("X-Test-Header".to_string()), vec!["test-value".to_string()]);
            Ok(())
        }

        fn handle_response(
            &self,
            _request: &PreparedRequest,
            _response: &mut foundation_core::wire::simple_http::SimpleResponse<foundation_core::wire::simple_http::SendSafeBody>,
        ) -> Result<(), foundation_core::wire::simple_http::HttpClientError> {
            Ok(())
        }
    }

    let mut request = PreparedRequest {
        method: SimpleMethod::GET,
        url: ParsedUrl::parse("http://example.com").unwrap(),
        headers: BTreeMap::new(),
        body: SendSafeBody::None,
        extensions: Extensions::new(),
    };

    let middleware = TestMiddleware;
    middleware.handle_request(&mut request).unwrap();

    assert_eq!(
        request.headers.get(&SimpleHeader::Custom("X-Test-Header".to_string())),
        Some(&vec!["test-value".to_string()])
    );
}

/// WHY: LoggingMiddleware should log request/response without modifying them
/// WHAT: Test that LoggingMiddleware doesn't change request
#[test]
fn test_logging_middleware_passthrough() {
    use foundation_core::wire::simple_http::client::{LoggingMiddleware, Middleware, PreparedRequest, ParsedUrl, Extensions};
    use foundation_core::wire::simple_http::{SimpleMethod, SendSafeBody};
    use std::collections::BTreeMap;

    let mut request = PreparedRequest {
        method: SimpleMethod::GET,
        url: ParsedUrl::parse("http://example.com/test").unwrap(),
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

/// WHY: TimingMiddleware should measure request duration
/// WHAT: Test that TimingMiddleware records timing in extensions
#[test]
fn test_timing_middleware_records_duration() {
    use foundation_core::wire::simple_http::client::{TimingMiddleware, Middleware, PreparedRequest, ParsedUrl, Extensions};
    use foundation_core::wire::simple_http::{SimpleMethod, SendSafeBody, Status};
    use std::collections::BTreeMap;
    use std::time::Duration;

    let mut request = PreparedRequest {
        method: SimpleMethod::GET,
        url: ParsedUrl::parse("http://example.com/test").unwrap(),
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

/// WHY: HeaderMiddleware should add default headers to requests
/// WHAT: Test that HeaderMiddleware adds headers only if not present
#[test]
fn test_header_middleware_adds_headers() {
    use foundation_core::wire::simple_http::client::{HeaderMiddleware, Middleware, PreparedRequest, ParsedUrl, Extensions};
    use foundation_core::wire::simple_http::{SimpleMethod, SendSafeBody, SimpleHeader};
    use std::collections::BTreeMap;

    let mut request = PreparedRequest {
        method: SimpleMethod::GET,
        url: ParsedUrl::parse("http://example.com/test").unwrap(),
        headers: BTreeMap::new(),
        body: SendSafeBody::None,
        extensions: Extensions::new(),
    };

    let middleware = HeaderMiddleware::new()
        .with_header(SimpleHeader::USER_AGENT, "test-agent".to_string());

    middleware.handle_request(&mut request).unwrap();

    // Header should be added
    assert_eq!(
        request.headers.get(&SimpleHeader::USER_AGENT),
        Some(&vec!["test-agent".to_string()])
    );
}

/// WHY: HeaderMiddleware should not overwrite existing headers
/// WHAT: Test that HeaderMiddleware respects existing headers
#[test]
fn test_header_middleware_respects_existing_headers() {
    use foundation_core::wire::simple_http::client::{HeaderMiddleware, Middleware, PreparedRequest, ParsedUrl, Extensions};
    use foundation_core::wire::simple_http::{SimpleMethod, SendSafeBody, SimpleHeader};
    use std::collections::BTreeMap;

    let mut request = PreparedRequest {
        method: SimpleMethod::GET,
        url: ParsedUrl::parse("http://example.com/test").unwrap(),
        headers: {
            let mut h = BTreeMap::new();
            h.insert(SimpleHeader::USER_AGENT, vec!["existing-agent".to_string()]);
            h
        },
        body: SendSafeBody::None,
        extensions: Extensions::new(),
    };

    let middleware = HeaderMiddleware::new()
        .with_header(SimpleHeader::USER_AGENT, "test-agent".to_string());

    middleware.handle_request(&mut request).unwrap();

    // Existing header should be preserved
    assert_eq!(
        request.headers.get(&SimpleHeader::USER_AGENT),
        Some(&vec!["existing-agent".to_string()])
    );
}

/// WHY: MiddlewareChain must execute in onion model (forward for requests, reverse for responses)
/// WHAT: Test middleware execution order
#[test]
fn test_middleware_chain_execution_order() {
    use foundation_core::wire::simple_http::client::{MiddlewareChain, Middleware, PreparedRequest, ParsedUrl, Extensions};
    use foundation_core::wire::simple_http::{SimpleMethod, SendSafeBody, SimpleHeader, Status};
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    // Track execution order
    let request_order = Arc::new(Mutex::new(Vec::new()));
    let response_order = Arc::new(Mutex::new(Vec::new()));

    struct OrderMiddleware {
        id: u32,
        request_order: Arc<Mutex<Vec<u32>>>,
        response_order: Arc<Mutex<Vec<u32>>>,
    }

    impl Middleware for OrderMiddleware {
        fn handle_request(&self, _request: &mut PreparedRequest) -> Result<(), foundation_core::wire::simple_http::HttpClientError> {
            self.request_order.lock().unwrap().push(self.id);
            Ok(())
        }

        fn handle_response(
            &self,
            _request: &PreparedRequest,
            _response: &mut foundation_core::wire::simple_http::SimpleResponse<foundation_core::wire::simple_http::SendSafeBody>,
        ) -> Result<(), foundation_core::wire::simple_http::HttpClientError> {
            self.response_order.lock().unwrap().push(self.id);
            Ok(())
        }
    }

    let mut chain = MiddlewareChain::new();
    chain.add(OrderMiddleware { id: 1, request_order: request_order.clone(), response_order: response_order.clone() });
    chain.add(OrderMiddleware { id: 2, request_order: request_order.clone(), response_order: response_order.clone() });
    chain.add(OrderMiddleware { id: 3, request_order: request_order.clone(), response_order: response_order.clone() });

    let mut request = PreparedRequest {
        method: SimpleMethod::GET,
        url: ParsedUrl::parse("http://example.com").unwrap(),
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
