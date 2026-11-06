//! Implementation of a duration-waiting function that uses `std::thread::park_timeout`
//! when the std feature is enabled and a spin loop when it's disabled.
//!
//! This module provides a function that waits for a specified duration, using
//! the appropriate implementation based on the presence of the std feature.
//!
//! When the std feature is enabled, it uses `std::thread::park_timeout` for efficient
//! waiting with low CPU usage. The elapsed time comes from the injected
//! [`TimeSource`], never from the global clock.
//!
//! When the std feature is disabled, it uses a spin loop with exponential backoff
//! to wait for the duration, which is suitable for `no_std` environments.

use core::time::Duration;

/// A monotonic time source for a blocking wait, behind the `std` feature.
///
/// The caller injects the source. A host passes its real clock. A test passes a
/// source it moves by hand, so the wait is deterministic. The engine's
/// `Clock` sits above this crate, which is why this crate names its own small
/// trait: a dependency on the engine would be a cycle.
#[cfg(feature = "std")]
pub trait TimeSource {
    /// The current monotonic instant.
    fn now(&self) -> std::time::Instant;
}

/// Waits for the specified duration (std build).
///
/// This function waits for the given duration, using `std::thread::park_timeout`.
/// The elapsed time comes from `clock`, not from the global clock.
///
/// # Arguments
///
/// * `dur` - The duration to wait for
/// * `clock` - The injected monotonic time source
///
/// # Examples
///
/// ```ignore
/// use foundation_nostd::primitives::wait_duration::wait_duration;
/// use core::time::Duration;
///
/// // `clock` is the injected `TimeSource`.
/// wait_duration(Duration::from_secs(1), clock);
/// ```
#[cfg(feature = "std")]
#[inline]
pub fn wait_duration(dur: Duration, clock: &dyn TimeSource) {
    let start = clock.now();

    let mut remaining_duration = dur;
    loop {
        let elapsed = clock.now().saturating_duration_since(start);
        if elapsed >= dur {
            break;
        }

        // Handle the case where park_timeout might be interrupted by a signal or
        // other event. The loop re-reads the injected clock, so a spurious wake
        // does not end the wait early.
        std::thread::park_timeout(remaining_duration);
        remaining_duration = remaining_duration
            .checked_sub(elapsed)
            .unwrap_or(Duration::new(0, 0));
    }
}

/// Waits for the specified duration (no-std build).
///
/// Uses a spin loop with exponential backoff. A no-std build has no monotonic
/// clock type, so this variant takes no time source.
///
/// # Examples
///
/// ```ignore
/// use foundation_nostd::primitives::wait_duration::wait_duration;
/// use core::time::Duration;
///
/// wait_duration(Duration::from_secs(1));
/// ```
#[cfg(not(feature = "std"))]
#[inline]
pub fn wait_duration(dur: Duration) {
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
