//! Client classifier tests (moved from `src/shared/http/client_classifier.rs`).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use foundation_netio::shared::http::client_classifier::{
    ClassificationThresholds, ClientClassification, ClientClassifier, SecurityLogger,
};

#[test]
fn test_classification_multipliers() {
    assert_eq!(ClientClassification::Normal.timeout_multiplier(), 1.0);
    assert_eq!(ClientClassification::Slow.timeout_multiplier(), 1.5);
    assert_eq!(ClientClassification::Suspicious.timeout_multiplier(), 0.5);
}

#[test]
fn test_client_classifier_new() {
    let classifier = ClientClassifier::new();
    assert_eq!(classifier.client_count(), 0);
}

#[test]
fn test_record_normal_transfer() {
    let classifier = ClientClassifier::new();

    // 100 KB in 1 second = 100 KB/s (normal)
    classifier.record_transfer("192.168.1.1", 100 * 1024, Duration::from_secs(1));

    assert_eq!(
        classifier.classify("192.168.1.1"),
        ClientClassification::Normal
    );
    assert_eq!(classifier.timeout_multiplier("192.168.1.1"), 1.0);
}

#[test]
fn test_record_slow_transfer() {
    let classifier = ClientClassifier::new();

    // 5 KB in 1 second = 5 KB/s (slow)
    classifier.record_transfer("192.168.1.1", 5 * 1024, Duration::from_secs(1));

    assert_eq!(
        classifier.classify("192.168.1.1"),
        ClientClassification::Slow
    );
    assert_eq!(classifier.timeout_multiplier("192.168.1.1"), 1.5);
}

#[test]
fn test_record_suspicious_transfer() {
    let classifier = ClientClassifier::new();

    // 500 bytes in 1 second = 0.5 KB/s (suspicious)
    classifier.record_transfer("192.168.1.1", 500, Duration::from_secs(1));

    assert_eq!(
        classifier.classify("192.168.1.1"),
        ClientClassification::Suspicious
    );
    assert_eq!(classifier.timeout_multiplier("192.168.1.1"), 0.5);
}

#[test]
fn test_consecutive_slow_penalty() {
    let classifier = ClientClassifier::new();

    // Multiple slow transfers
    classifier.record_transfer("192.168.1.1", 5 * 1024, Duration::from_secs(1));
    classifier.record_transfer("192.168.1.1", 5 * 1024, Duration::from_secs(1));
    classifier.record_transfer("192.168.1.1", 5 * 1024, Duration::from_secs(1));

    let stats = classifier.get_stats("192.168.1.1").unwrap();
    assert_eq!(stats.consecutive_slow, 3);
    assert_eq!(stats.transfer_count, 3);
}

#[test]
fn test_rehabilitation() {
    let classifier = ClientClassifier::new();

    // Start slow
    classifier.record_transfer("192.168.1.1", 5 * 1024, Duration::from_secs(1));
    assert_eq!(
        classifier.classify("192.168.1.1"),
        ClientClassification::Slow
    );

    // Then normal - should reset
    classifier.record_transfer("192.168.1.1", 100 * 1024, Duration::from_secs(1));
    assert_eq!(
        classifier.classify("192.168.1.1"),
        ClientClassification::Normal
    );

    let stats = classifier.get_stats("192.168.1.1").unwrap();
    assert_eq!(stats.consecutive_slow, 0);
}

#[test]
fn test_stats_calculation() {
    let classifier = ClientClassifier::new();

    classifier.record_transfer("192.168.1.1", 10 * 1024, Duration::from_secs(1));
    classifier.record_transfer("192.168.1.1", 10 * 1024, Duration::from_secs(1));

    let stats = classifier.get_stats("192.168.1.1").unwrap();
    assert_eq!(stats.total_bytes, 20 * 1024);
    assert_eq!(stats.transfer_count, 2);
    assert!((stats.average_rate_kbps() - 10.0).abs() < 0.1);
}

#[test]
fn test_unknown_client() {
    let classifier = ClientClassifier::new();

    // Unknown clients are treated as Normal
    assert_eq!(classifier.classify("unknown"), ClientClassification::Normal);
    assert_eq!(classifier.timeout_multiplier("unknown"), 1.0);
}

#[test]
fn test_remove_client() {
    let classifier = ClientClassifier::new();

    classifier.record_transfer("192.168.1.1", 10240, Duration::from_secs(1));
    assert_eq!(classifier.client_count(), 1);

    classifier.remove_client("192.168.1.1");
    assert_eq!(classifier.client_count(), 0);
}

#[test]
fn test_clear() {
    let classifier = ClientClassifier::new();

    classifier.record_transfer("192.168.1.1", 10240, Duration::from_secs(1));
    classifier.record_transfer("192.168.1.2", 10240, Duration::from_secs(1));
    assert_eq!(classifier.client_count(), 2);

    classifier.clear();
    assert_eq!(classifier.client_count(), 0);
}

#[test]
fn test_zero_duration() {
    let classifier = ClientClassifier::new();

    // Zero duration should be ignored
    classifier.record_transfer("192.168.1.1", 10240, Duration::ZERO);

    assert_eq!(classifier.client_count(), 0);
}

#[test]
fn test_custom_thresholds() {
    let thresholds = ClassificationThresholds {
        normal_threshold: 100.0, // 100 KB/s
        slow_threshold: 50.0,    // 50 KB/s
        suspicious_after: 2,
        client_ttl: Duration::from_secs(3600),
    };
    let classifier = ClientClassifier::with_thresholds(thresholds);

    // 75 KB/s is slow with custom thresholds
    classifier.record_transfer("192.168.1.1", 75 * 1024, Duration::from_secs(1));
    assert_eq!(
        classifier.classify("192.168.1.1"),
        ClientClassification::Slow
    );
}

// Mock security logger for testing
struct MockSecurityLogger {
    call_count: AtomicUsize,
}

impl SecurityLogger for MockSecurityLogger {
    fn log_suspicious(&self, _client_ip: &str, _rate: f64, _consecutive: u32) {
        self.call_count.fetch_add(1, Ordering::Relaxed);
    }
}

#[test]
fn test_security_logger() {
    let logger = Arc::new(MockSecurityLogger {
        call_count: AtomicUsize::new(0),
    });

    let thresholds = ClassificationThresholds {
        normal_threshold: 10.0,
        slow_threshold: 1.0,
        suspicious_after: 2,
        client_ttl: Duration::from_secs(3600),
    };

    let classifier =
        ClientClassifier::with_thresholds(thresholds).with_security_logger(logger.clone());

    // Multiple suspicious transfers
    classifier.record_transfer("192.168.1.1", 500, Duration::from_secs(1));
    classifier.record_transfer("192.168.1.1", 500, Duration::from_secs(1));
    classifier.record_transfer("192.168.1.1", 500, Duration::from_secs(1));

    // Should have logged at least once
    assert!(logger.call_count.load(Ordering::Relaxed) > 0);
}
