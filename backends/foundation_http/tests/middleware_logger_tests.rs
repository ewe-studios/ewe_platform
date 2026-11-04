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
