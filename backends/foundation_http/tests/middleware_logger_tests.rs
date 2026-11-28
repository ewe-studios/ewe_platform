//! Logger middleware tests (moved from `src/shared/middleware/logger.rs`).

use foundation_http::shared::middleware::{LogLevel, LoggerConfig, LoggerMiddleware};

#[test]
fn test_logger_config_default() {
    let config = LoggerConfig::default();
    assert_eq!(config.level, LogLevel::Info);
    assert!(config.include_client_ip);
    assert!(!config.include_headers);
    assert!(config.include_query);
    assert!(config.log_response_time);
}

#[test]
fn test_logger_middleware_info() {
    let mw = LoggerMiddleware::info();
    assert_eq!(mw.level(), LogLevel::Info);
}

#[test]
fn test_logger_middleware_debug() {
    let mw = LoggerMiddleware::debug();
    assert_eq!(mw.level(), LogLevel::Debug);
}

#[tracing_test::traced_test]
#[test]
fn logger_includes_query_only_in_its_enabled_field() {
    use foundation_http::shared::context::Ctx;
    use foundation_http::shared::middleware::RequestMiddleware;
    use foundation_http::{SimpleIncomingRequest, SimpleMethod};

    let middleware = LoggerMiddleware::new(LoggerConfig {
        include_query: false,
        ..LoggerConfig::default()
    });
    let mut request = SimpleIncomingRequest::builder()
        .with_parsed_url("/example?field=synthetic-logger-marker")
        .with_method(SimpleMethod::GET)
        .build()
        .expect("valid request");
    let ctx = Ctx::host();
    middleware.handle(&ctx, &mut request);

    assert!(logs_contain("GET /example"));
    assert!(!logs_contain("GET /example?field="));
    assert!(!logs_contain("query=?field=synthetic-logger-marker"));

    LoggerMiddleware::defaults().handle(&ctx, &mut request);
    assert!(!logs_contain("GET /example?field=synthetic-logger-marker"));
    assert!(logs_contain("query=?field=synthetic-logger-marker"));
}
