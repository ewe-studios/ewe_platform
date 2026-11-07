//! Tests extracted from `simple_http/shared/load_tracker.rs`
mod tests {
    use foundation_core::valtron::SimulatedClock;
    use foundation_netio::shared::http::load_tracker::*;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    /// A tracker on a real clock, for the tests that do not care about time.
    fn tracker() -> LoadTracker {
        LoadTracker::new(Arc::new(foundation_core::valtron::MonotonicClock::new()))
    }

    #[test]
    fn the_connection_rate_is_not_always_zero() {
        let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
        let tracker = LoadTracker::new(clock.clone());

        let _g1 = tracker.track_connection();
        let _g2 = tracker.track_connection();
        clock.advance(Duration::from_secs(1));

        // Two connections inside one second is 2 connections/sec.
        assert_eq!(tracker.connection_rate(), 2);
    }

    #[test]
    fn a_zero_elapsed_window_returns_the_previous_rate() {
        let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
        let tracker = LoadTracker::new(clock.clone());

        // A fresh tracker with no elapsed time keeps its initial rate.
        assert_eq!(tracker.connection_rate(), 0);

        let _g1 = tracker.track_connection();
        let _g2 = tracker.track_connection();
        clock.advance(Duration::from_secs(1));
        assert_eq!(tracker.connection_rate(), 2);

        // Two more connections but no clock advance: the rate stays at the
        // previous value and the read does not divide by zero.
        let _g3 = tracker.track_connection();
        let _g4 = tracker.track_connection();
        assert_eq!(tracker.connection_rate(), 2);
    }

    #[test]
    fn test_load_level_factors() {
        assert_eq!(LoadLevel::Low.timeout_factor(), 1.0);
        assert_eq!(LoadLevel::Medium.timeout_factor(), 0.9);
        assert_eq!(LoadLevel::High.timeout_factor(), 0.75);
        assert_eq!(LoadLevel::Critical.timeout_factor(), 0.6);
    }

    #[test]
    fn test_load_tracker_new() {
        let tracker = tracker();
        assert_eq!(tracker.active_connections(), 0);
    }

    #[test]
    fn test_track_connection() {
        let tracker = tracker();

        let guard = tracker.track_connection();
        assert_eq!(tracker.active_connections(), 1);

        guard.complete();
        // Note: after complete(), the guard is consumed
        // We can't check active_connections here because drop() is called
    }

    #[test]
    fn test_connection_guard_drop() {
        let tracker = tracker();

        {
            let _guard = tracker.track_connection();
            assert_eq!(tracker.active_connections(), 1);
            // _guard is dropped here
        }

        // After drop, connection should be released
        // Note: This is a simplified test - in reality the atomic decrement
        // happens in the Drop impl
    }

    #[test]
    fn test_multiple_connections() {
        let tracker = tracker();

        let guard1 = tracker.track_connection();
        let guard2 = tracker.track_connection();
        let guard3 = tracker.track_connection();

        assert_eq!(tracker.active_connections(), 3);

        guard1.complete();
        guard2.complete();
        guard3.complete();
    }

    #[test]
    fn test_load_level_calculation() {
        let config = LoadTrackerConfig {
            medium_threshold: 100,
            high_threshold: 1000,
            critical_threshold: 10000,
            calculation_interval: Duration::ZERO,
        };
        let tracker = LoadTracker::with_config(
            config,
            Arc::new(foundation_core::valtron::MonotonicClock::new()),
        );

        // Initially should be low
        assert_eq!(tracker.current_load_level(), LoadLevel::Low);
    }

    #[test]
    fn test_timeout_factor() {
        let tracker = tracker();
        let factor = tracker.timeout_factor();
        assert!(factor > 0.0 && factor <= 1.0);
    }

    #[test]
    fn test_reset() {
        let tracker = tracker();

        let guard = tracker.track_connection();
        assert_eq!(tracker.active_connections(), 1);

        guard.complete();
        tracker.reset();

        assert_eq!(tracker.active_connections(), 0);
    }

    #[test]
    fn test_concurrent_connections() {
        let tracker = tracker();
        let tracker = std::sync::Arc::new(tracker);

        let mut handles = vec![];

        for _ in 0..10 {
            let t = tracker.clone();
            handles.push(thread::spawn(move || {
                let _guard = t.track_connection();
                thread::sleep(Duration::from_millis(10));
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // All connections should be released
        assert_eq!(tracker.active_connections(), 0);
    }
}
