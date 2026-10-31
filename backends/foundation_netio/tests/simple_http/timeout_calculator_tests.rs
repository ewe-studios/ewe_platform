//! Timeout calculator and configuration tests (moved from `src/shared/http/timeout.rs`).

use std::time::Duration;

use foundation_netio::shared::http::timeout::{
    TimeoutCalculator, TimeoutConfig, TimeoutContext,
};

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
    let ctx = TimeoutContext::with_size((36u64 * 1024 * 1024 * 1024) as usize);
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
    let ctx = TimeoutContext::with_size((75u64 * 1024 * 1024 * 1024) as usize);
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
    let ctx = TimeoutContext::with_size((9u64 * 1024 * 1024 * 1024) as usize);
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
