//! Client classification for DoS protection and resource fairness.
//!
//! WHY: Slow clients (slowloris attacks, poor connections) can exhaust server
//! resources. By classifying clients by transfer rate, we can apply progressive
//! penalties and protect server capacity.
//!
//! WHAT: ClientClassifier tracks transfer rates and classifies clients as:
//! - Normal (>10 KB/s): No penalty
//! - Slow (1-10 KB/s): +50% timeout (tolerate slow connections)
//! - Suspicious (<1 KB/s): -50% timeout (fail fast, log security)
//!
//! HOW: Per-client stats track bytes transferred and duration. Rate is calculated
//! on each request completion. Progressive penalties are applied for consecutive
//! slow transfers.
//!
//! # Example
//!
//! ```
//! use foundation_core::wire::simple_http::client_classifier::ClientClassifier;
//! use std::time::Duration;
//!
//! let classifier = ClientClassifier::new();
//!
//! // Record a transfer
//! classifier.record_transfer("192.168.1.1", 10240, Duration::from_secs(1));
//!
//! // Get classification
//! let classification = classifier.classify("192.168.1.1");
//! println!("Classification: {:?}", classification);
//! ```

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Client classification based on transfer rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClientClassification {
    /// Normal client (>10 KB/s).
    Normal,
    /// Slow client (1-10 KB/s).
    Slow,
    /// Suspicious client (<1 KB/s).
    Suspicious,
}

impl ClientClassification {
    /// Get the timeout multiplier for this classification.
    #[must_use]
    pub fn timeout_multiplier(&self) -> f64 {
        match self {
            ClientClassification::Normal => 1.0,
            ClientClassification::Slow => 1.5,   // +50% timeout
            ClientClassification::Suspicious => 0.5, // -50% timeout (fail fast)
        }
    }

    /// Get description of this classification.
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            ClientClassification::Normal => "Normal transfer rate",
            ClientClassification::Slow => "Slow transfer rate",
            ClientClassification::Suspicious => "Suspicious transfer rate",
        }
    }
}

/// Configuration for client classification thresholds.
#[derive(Debug, Clone, Copy)]
pub struct ClassificationThresholds {
    /// Threshold for normal classification (KB/s).
    pub normal_threshold: f64,
    /// Threshold for slow classification (KB/s).
    pub slow_threshold: f64,
    /// Number of consecutive slow transfers before suspicious.
    pub suspicious_after: u32,
    /// Client TTL before removal.
    pub client_ttl: Duration,
}

impl Default for ClassificationThresholds {
    fn default() -> Self {
        Self {
            normal_threshold: 10.0, // 10 KB/s
            slow_threshold: 1.0,     // 1 KB/s
            suspicious_after: 3,     // 3 consecutive slow transfers
            client_ttl: Duration::from_secs(3600), // 1 hour
        }
    }
}

/// Statistics for a tracked client.
#[derive(Debug, Clone)]
pub struct ClientStats {
    /// Current classification.
    pub classification: ClientClassification,
    /// Total bytes transferred.
    pub total_bytes: usize,
    /// Total transfer duration.
    pub total_duration: Duration,
    /// Consecutive slow transfers.
    pub consecutive_slow: u32,
    /// Last transfer time.
    pub last_transfer: Instant,
    /// Number of transfers.
    pub transfer_count: u32,
}

impl ClientStats {
    /// Create new stats with initial classification.
    #[must_use]
    pub fn new(classification: ClientClassification) -> Self {
        Self {
            classification,
            total_bytes: 0,
            total_duration: Duration::ZERO,
            consecutive_slow: 0,
            last_transfer: Instant::now(),
            transfer_count: 0,
        }
    }

    /// Calculate average transfer rate (KB/s).
    #[must_use]
    pub fn average_rate_kbps(&self) -> f64 {
        if self.total_duration.as_secs_f64() == 0.0 {
            return 0.0;
        }
        let rate = self.total_bytes as f64 / self.total_duration.as_secs_f64();
        rate / 1024.0
    }
}

/// Security logger trait for suspicious clients.
pub trait SecurityLogger: Send + Sync {
    /// Log a suspicious client.
    fn log_suspicious(&self, client_ip: &str, rate: f64, consecutive: u32);
}

/// Thread-safe client classifier.
#[derive(Clone)]
pub struct ClientClassifier {
    /// Per-client stats.
    clients: Arc<RwLock<HashMap<String, ClientStats>>>,
    /// Classification thresholds.
    thresholds: ClassificationThresholds,
    /// Security logger (optional).
    security_logger: Option<Arc<dyn SecurityLogger>>,
}

impl std::fmt::Debug for ClientClassifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientClassifier")
            .field("clients", &self.clients)
            .field("thresholds", &self.thresholds)
            .field("has_security_logger", &self.security_logger.is_some())
            .finish()
    }
}

impl ClientClassifier {
    /// Create a new client classifier with default thresholds.
    #[must_use]
    pub fn new() -> Self {
        Self::with_thresholds(ClassificationThresholds::default())
    }

    /// Create a new classifier with custom thresholds.
    #[must_use]
    pub fn with_thresholds(thresholds: ClassificationThresholds) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            thresholds,
            security_logger: None,
        }
    }

    /// Set the security logger.
    #[must_use]
    pub fn with_security_logger(mut self, logger: Arc<dyn SecurityLogger>) -> Self {
        self.security_logger = Some(logger);
        self
    }

    /// Record a transfer and update classification.
    pub fn record_transfer(&self, client_ip: &str, bytes: usize, duration: Duration) {
        if duration.is_zero() {
            return;
        }

        let rate_kbps = (bytes as f64 / duration.as_secs_f64()) / 1024.0;
        let classification = self.classify_rate(rate_kbps);

        let mut clients = self.clients.write().expect("poisoned lock");
        let stats = clients
            .entry(client_ip.to_string())
            .or_insert_with(|| ClientStats::new(classification));

        // Update stats
        stats.total_bytes += bytes;
        stats.total_duration += duration;
        stats.transfer_count += 1;
        stats.last_transfer = Instant::now();

        // Handle progressive penalties
        match classification {
            ClientClassification::Normal => {
                stats.consecutive_slow = 0;
                stats.classification = ClientClassification::Normal;
            }
            ClientClassification::Slow => {
                stats.consecutive_slow += 1;
                stats.classification = ClientClassification::Slow;
            }
            ClientClassification::Suspicious => {
                stats.consecutive_slow += 1;
                stats.classification = ClientClassification::Suspicious;

                // Log if suspicious threshold reached
                if stats.consecutive_slow >= self.thresholds.suspicious_after {
                    if let Some(ref logger) = self.security_logger {
                        logger.log_suspicious(client_ip, rate_kbps, stats.consecutive_slow);
                    }
                }
            }
        }
    }

    /// Get current classification for a client.
    #[must_use]
    pub fn classify(&self, client_ip: &str) -> ClientClassification {
        let clients = self.clients.read().expect("poisoned lock");
        clients
            .get(client_ip)
            .map(|s| s.classification)
            .unwrap_or(ClientClassification::Normal)
    }

    /// Get timeout multiplier for a client.
    #[must_use]
    pub fn timeout_multiplier(&self, client_ip: &str) -> f64 {
        self.classify(client_ip).timeout_multiplier()
    }

    /// Get stats for a client.
    #[must_use]
    pub fn get_stats(&self, client_ip: &str) -> Option<ClientStats> {
        let clients = self.clients.read().expect("poisoned lock");
        clients.get(client_ip).cloned()
    }

    /// Check if client has been classified.
    #[must_use]
    pub fn is_tracked(&self, client_ip: &str) -> bool {
        let clients = self.clients.read().expect("poisoned lock");
        clients.contains_key(client_ip)
    }

    /// Remove a client from tracking.
    pub fn remove_client(&self, client_ip: &str) {
        let mut clients = self.clients.write().expect("poisoned lock");
        clients.remove(client_ip);
    }

    /// Clear all clients.
    pub fn clear(&self) {
        let mut clients = self.clients.write().expect("poisoned lock");
        clients.clear();
    }

    /// Get number of tracked clients.
    #[must_use]
    pub fn client_count(&self) -> usize {
        let clients = self.clients.read().expect("poisoned lock");
        clients.len()
    }

    /// Cleanup old clients.
    pub fn cleanup(&self) {
        let now = Instant::now();
        let mut clients = self.clients.write().expect("poisoned lock");
        clients.retain(|_, stats| {
            now.duration_since(stats.last_transfer) < self.thresholds.client_ttl
        });
    }

    /// Classify rate based on thresholds.
    fn classify_rate(&self, rate_kbps: f64) -> ClientClassification {
        if rate_kbps >= self.thresholds.normal_threshold {
            ClientClassification::Normal
        } else if rate_kbps >= self.thresholds.slow_threshold {
            ClientClassification::Slow
        } else {
            ClientClassification::Suspicious
        }
    }
}

impl Default for ClientClassifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

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

        assert_eq!(classifier.classify("192.168.1.1"), ClientClassification::Normal);
        assert_eq!(classifier.timeout_multiplier("192.168.1.1"), 1.0);
    }

    #[test]
    fn test_record_slow_transfer() {
        let classifier = ClientClassifier::new();

        // 5 KB in 1 second = 5 KB/s (slow)
        classifier.record_transfer("192.168.1.1", 5 * 1024, Duration::from_secs(1));

        assert_eq!(classifier.classify("192.168.1.1"), ClientClassification::Slow);
        assert_eq!(classifier.timeout_multiplier("192.168.1.1"), 1.5);
    }

    #[test]
    fn test_record_suspicious_transfer() {
        let classifier = ClientClassifier::new();

        // 500 bytes in 1 second = 0.5 KB/s (suspicious)
        classifier.record_transfer("192.168.1.1", 500, Duration::from_secs(1));

        assert_eq!(classifier.classify("192.168.1.1"), ClientClassification::Suspicious);
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
        assert_eq!(classifier.classify("192.168.1.1"), ClientClassification::Slow);

        // Then normal - should reset
        classifier.record_transfer("192.168.1.1", 100 * 1024, Duration::from_secs(1));
        assert_eq!(classifier.classify("192.168.1.1"), ClientClassification::Normal);

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
        assert_eq!(classifier.classify("192.168.1.1"), ClientClassification::Slow);
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

        let classifier = ClientClassifier::with_thresholds(thresholds)
            .with_security_logger(logger.clone());

        // Multiple suspicious transfers
        classifier.record_transfer("192.168.1.1", 500, Duration::from_secs(1));
        classifier.record_transfer("192.168.1.1", 500, Duration::from_secs(1));
        classifier.record_transfer("192.168.1.1", 500, Duration::from_secs(1));

        // Should have logged at least once
        assert!(logger.call_count.load(Ordering::Relaxed) > 0);
    }
}
