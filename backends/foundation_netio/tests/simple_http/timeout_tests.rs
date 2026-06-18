/// WHY: Timeout system must have production-quality defaults to ensure proper behavior
/// WHAT: TimeoutConfig default values should match production configuration
#[test]
fn test_timeout_config_default_values() {
    use foundation_netio::simple_http::shared::timeout::TimeoutConfig;
    use std::time::Duration;

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
