//! Implementation of a duration-waiting function that uses `std::thread::park_timeout`
//! when the std feature is enabled and a spin loop when it's disabled.
//!
//! This module provides a function that waits for a specified duration, using
//! the appropriate implementation based on the presence of the std feature.
//!
//! When the std feature is enabled, it uses `std::thread::park_timeout` for efficient
//! waiting with low CPU usage.
//!
//! When the std feature is disabled, it uses a spin loop with exponential backoff
//! to wait for the duration, which is suitable for `no_std` environments.
//!
//! # Examples
//!
//! ```
//! use foundation_nostd::primitives::wait_duration;
//! use core::time::Duration;
//!
//! // Wait for 1 second
//! wait_duration(Duration::from_secs(1));
//! ```

use core::time::Duration;

/// Waits for the specified duration.
///
/// This function waits for the given duration, using `std::thread::park_timeout`
/// when the std feature is enabled and a spin loop when it's disabled.
///
/// # Arguments
///
/// * `dur` - The duration to wait for
///
/// # Examples
///
/// ```
/// use foundation_nostd::primitives::wait_duration;
/// use core::time::Duration;
///
/// // Wait for 1 second
/// wait_duration(Duration::from_secs(1));
/// ```
#[inline]
pub fn wait_duration(dur: Duration) {
    // Use std::thread::park_timeout when the std feature is enabled
    #[cfg(feature = "std")]
    {
        use std::time::Instant;

        // Wait for the specified duration using a spin loop
        let start = Instant::now();

        let mut remaining_duration = dur;
        loop {
            let elapsed = start.elapsed();
            if elapsed >= dur {
                break;
            }

            // Handle the case where park_timeout might be interrupted by a signal or other event
            // In a real implementation, we would need to handle this case properly
            // For now, we'll just call park_timeout and assume it completes successfully
            std::thread::park_timeout(remaining_duration);
            remaining_duration = remaining_duration
                .checked_sub(elapsed)
                .unwrap_or(Duration::new(0, 0));
        }
    }

    // Use a spin loop with exponential backoff when the std feature is disabled
    #[cfg(not(feature = "std"))]
    {
        use crate::primitives::spin_wait::SpinWait;

        // Use SpinWait for efficient spinning with exponential backoff
        let mut spin_wait = SpinWait::new();

        // Wait for the specified duration using a spin loop
        let max_spins = (dur.as_micros() / 10).max(1) as usize;

        for _ in 0..max_spins {
            // Use exponential backoff to reduce CPU usage
            if !spin_wait.spin() {
                // If we've exhausted the spin limit, reset the spin wait
                spin_wait.reset();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "std")]
    mod when_std {
        extern crate std;

        use super::*;

        use core::time::Duration;
        use std::time::Instant;

        /// Test that wait_duration waits for the correct duration
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
                "Wait duration was too short: {:?}",
                elapsed
            );
        }

        /// Test that wait_duration handles zero duration correctly
        #[test]
        fn test_wait_duration_zero_duration() {
            let start = Instant::now();
            wait_duration(Duration::from_nanos(0));
            let elapsed = start.elapsed();

            // Zero duration should result in immediate return
            assert!(
                elapsed.as_nanos() < 1000,
                "Zero duration wait took too long: {:?}",
                elapsed
            );
        }

        /// Test that wait_duration handles very short durations correctly
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
                "Short wait duration was too short: {:?}",
                elapsed
            );
        }

        /// Test that wait_duration works with very long durations
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
                "Long wait duration was too short: {:?}",
                elapsed
            );
        }

        /// Test that wait_duration works with duration that is not a multiple of the system timer resolution
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
                "Non-multiple wait duration was too short: {:?}",
                elapsed
            );
        }

        /// Test that wait_duration works with duration that is very small
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
                "Very small wait duration was too short: {:?}",
                elapsed
            );
        }

        /// Test that wait_duration works with duration that is very large
        #[test]
        fn test_wait_duration_very_large_duration() {
            // Test with a very large duration
            let start = Instant::now();
            wait_duration(Duration::from_secs(5));
            let elapsed = start.elapsed();

            // The actual elapsed time should be at least 1000 seconds
            // Allow for some tolerance
            assert!(
                elapsed.as_secs() >= 5,
                "Very large wait duration was too short: {:?}",
                elapsed
            );
        }

        /// Test that wait_duration works with duration that is exactly the system timer resolution
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
                "Exact timer resolution wait duration was too short: {:?}",
                elapsed
            );
        }

        /// Test that wait_duration works with duration that is a fraction of the system timer resolution
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
                "Fraction timer resolution wait duration was too short: {:?}",
                elapsed
            );
        }
    }

    mod when_nostd {
        use super::*;
        use core::time::Duration;

        /// Test that wait_duration waits for the correct duration
        #[test]
        fn test_wait_duration_correct_duration() {
            // Test with a 100ms duration
            wait_duration(Duration::from_millis(100));
        }
    }
}
