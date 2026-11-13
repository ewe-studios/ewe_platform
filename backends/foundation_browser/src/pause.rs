//! # Pause driven by the injected clock
//!
//! WHY: A wait loop reads its deadline from the injected [`Ctx`] clock. If the
//! loop paused with `std::thread::sleep`, a simulated clock would never move
//! and the loop would run forever. A real sleep also reads the real time, which
//! breaks the rule that side effects are injected.
//!
//! WHAT: [`WaitConfig`] holds the two wait intervals. [`pause`] blocks until
//! the `Ctx` clock has moved forward by the requested time.
//!
//! HOW: [`pause`] reads the start time from the `Ctx` clock. It parks the
//! thread for at most [`WaitConfig::wake_interval`], then reads the clock
//! again. A real clock moves by itself. A simulated clock moves only when a
//! test calls `advance`, so the test can step every wait.
//!
//! The driver API stays blocking. The `wasm_ui_server` macro generates
//! blocking tests, and `Page` and `Locator` block on a socket. An async
//! `launch` would force a change in every generated test, so the pause
//! stays blocking too.

use std::time::Duration;

use foundation_context::Ctx;

/// The two intervals that control every wait loop in the driver.
///
/// The defaults match the old fixed sleeps (50 ms between attempts, as in the
/// Playwright and Selenium poll loops).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaitConfig {
    /// Clock time to wait between two attempts. Default: 50 ms.
    pub poll_interval: Duration,
    /// The most real time the thread parks before it reads the clock again.
    /// A smaller value makes a simulated clock step faster. Default: 5 ms.
    pub wake_interval: Duration,
}

impl Default for WaitConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(50),
            wake_interval: Duration::from_millis(5),
        }
    }
}

impl WaitConfig {
    /// Set the clock time between two attempts.
    #[must_use]
    pub fn poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Set the most real time between two clock reads.
    #[must_use]
    pub fn wake_interval(mut self, interval: Duration) -> Self {
        self.wake_interval = interval;
        self
    }
}

/// Block until the `ctx` clock has moved forward by `duration`.
///
/// The thread parks for short real intervals and reads the clock after each.
/// With a frozen simulated clock the call does not return until a test
/// advances the clock.
pub fn pause(ctx: &Ctx, duration: Duration, wait: &WaitConfig) {
    let clock = ctx.clock();
    let start = clock.now();
    loop {
        let elapsed = clock
            .now()
            .checked_duration_since(start)
            .unwrap_or(Duration::ZERO);
        // Never park longer than the clock time left, and never for zero.
        let Some(remaining) = duration.checked_sub(elapsed).filter(|d| !d.is_zero()) else {
            return;
        };
        let park = remaining
            .min(wait.wake_interval)
            .max(Duration::from_micros(100));
        std::thread::park_timeout(park);
    }
}
