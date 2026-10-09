use foundation_nostd::primitives::wait_duration::*;

#[cfg(feature = "std")]
mod when_std {
    extern crate std;

    use foundation_nostd::primitives::wait_duration::*;
    use core::time::Duration;
    use std::time::Instant;

    /// Test that `wait_duration` waits for the correct duration
    #[test]
    fn test_wait_duration_correct_duration() {
        // Test with a 100ms duration
        let start = Instant::now();
        wait_duration(Duration::from_millis(100));
        let elapsed = start.elapsed();

        // The actual elapsed time should be at least 100ms
        // Allow for some tolerance due to system scheduling
        assert!(
            elapsed.as_millis() >= 100,
            "Wait duration was too short: {elapsed:?}"
        );
    }

    /// Test that `wait_duration` handles zero duration correctly
    #[test]
    fn test_wait_duration_zero_duration() {
        let start = Instant::now();
        wait_duration(Duration::from_nanos(0));
        let elapsed = start.elapsed();

        // Zero duration should result in immediate return
        assert!(
            elapsed.as_nanos() < 1000,
            "Zero duration wait took too long: {elapsed:?}"
        );
    }

    /// Test that `wait_duration` handles very short durations correctly
    #[test]
    fn test_wait_duration_short_duration() {
        // Test with a very short duration
        let start = Instant::now();
        wait_duration(Duration::from_micros(10));
        let elapsed = start.elapsed();

        // The actual elapsed time should be at least 10us
        // Allow for some tolerance
        assert!(
            elapsed.as_micros() >= 10,
            "Short wait duration was too short: {elapsed:?}"
        );
    }

    /// Test that `wait_duration` works with very long durations
    #[test]
    fn test_wait_duration_long_duration() {
        // Test with a 1 second duration
        let start = Instant::now();
        wait_duration(Duration::from_secs(1));
        let elapsed = start.elapsed();

        // The actual elapsed time should be at least 1 second
        // Allow for some tolerance
        assert!(
            elapsed.as_secs() >= 1,
            "Long wait duration was too short: {elapsed:?}"
        );
    }

    /// Test that `wait_duration` works with duration that is not a multiple of the system timer resolution
    #[test]
    fn test_wait_duration_non_multiple_duration() {
        // Test with a duration that is not a multiple of the system timer resolution
        let start = Instant::now();
        wait_duration(Duration::from_millis(123));
        let elapsed = start.elapsed();

        // The actual elapsed time should be at least 123ms
        // Allow for some tolerance
        assert!(
            elapsed.as_millis() >= 123,
            "Non-multiple wait duration was too short: {elapsed:?}"
        );
    }

    /// Test that `wait_duration` works with duration that is very small
    #[test]
    fn test_wait_duration_very_small_duration() {
        // Test with a very small duration
        let start = Instant::now();
        wait_duration(Duration::from_nanos(1));
        let elapsed = start.elapsed();

        // The actual elapsed time should be at least 1ns
        // Allow for some tolerance
        assert!(
            elapsed.as_nanos() >= 1,
            "Very small wait duration was too short: {elapsed:?}"
        );
    }

    /// Test that `wait_duration` works with duration that is very large
    #[test]
    fn test_wait_duration_very_large_duration() {
        // Test with a very large duration
        let start = Instant::now();
        wait_duration(Duration::from_secs(5));
        let elapsed = start.elapsed();

        // The actual elapsed time should be at least 5 seconds
        // Allow for some tolerance
        assert!(
            elapsed.as_secs() >= 5,
            "Very large wait duration was too short: {elapsed:?}"
        );
    }

    /// Test that `wait_duration` works with duration that is exactly the system timer resolution
    #[test]
    fn test_wait_duration_exact_timer_resolution() {
        // Test with a duration that is exactly the system timer resolution
        let start = Instant::now();
        wait_duration(Duration::from_millis(1));
        let elapsed = start.elapsed();

        // The actual elapsed time should be at least 1ms
        // Allow for some tolerance
        assert!(
            elapsed.as_millis() >= 1,
            "Exact timer resolution wait duration was too short: {elapsed:?}"
        );
    }

    /// Test that `wait_duration` works with duration that is a fraction of the system timer resolution
    #[test]
    fn test_wait_duration_fraction_timer_resolution() {
        // Test with a duration that is a fraction of the system timer resolution
        let start = Instant::now();
        wait_duration(Duration::from_micros(500));
        let elapsed = start.elapsed();

        // The actual elapsed time should be at least 500us
        // Allow for some tolerance
        assert!(
            elapsed.as_micros() >= 500,
            "Fraction timer resolution wait duration was too short: {elapsed:?}"
        );
    }
}

mod when_nostd {
    use foundation_nostd::primitives::wait_duration::*;
    use core::time::Duration;

    /// Test that `wait_duration` waits for the correct duration
    #[test]
    fn test_wait_duration_correct_duration() {
        // Test with a 100ms duration
        wait_duration(Duration::from_millis(100));
    }
}
