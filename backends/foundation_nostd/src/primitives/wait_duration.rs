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
