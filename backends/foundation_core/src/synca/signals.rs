use std::sync::atomic::{AtomicUsize, Ordering};

/// Indicates the underlying signal to be set.
const SET: usize = 1;

/// Indicates the underlying signal was not set.
const UNSET: usize = 0;

#[derive(Debug)]
pub struct OnSignal {
    state: AtomicUsize,
}

impl Default for OnSignal {
    fn default() -> Self {
        Self::new()
    }
}

impl OnSignal {
    pub fn new() -> Self {
        Self {
            state: AtomicUsize::new(0),
        }
    }

    /// `turn_off` flip the state from SET to UNSET.
    #[inline]
    pub fn turn_off(&self) -> bool {
        self.state
            .compare_exchange(SET, UNSET, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
    }

    /// turn_on flip the state from UNSET to SET.
    #[inline]
    pub fn turn_on(&self) -> bool {
        self.state
            .compare_exchange(UNSET, SET, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
    }

    /// probe returns true when the state is SET else false.
    #[inline]
    pub fn probe(&self) -> bool {
        self.state.load(Ordering::Acquire) == SET
    }
}

/// Signal is indicative that the owner signal is going to sleep.
const SLEEPY: usize = 2;

/// Signal is indicative that the owner signal is now sleeping.
const SLEEPING: usize = 3;

/// `ActivitySignal` allows you to move a SleepSignal from
/// a series of steps:
///
/// 1. UNSET = indicate the signal is not asleep nor set.
///
/// 2. SLEEPY = indicate the signal is getting to a sleepy state.
///
/// 3. SLEEPING = indicate the signal is now is sleeping state.
///
/// 4. SET = indicate the signal is now is considered inactive.
///
#[derive(Debug)]
pub struct ActivitySignal {
    state: AtomicUsize,
}

impl Default for ActivitySignal {
    fn default() -> Self {
        Self::new()
    }
}

impl ActivitySignal {
    pub fn new() -> Self {
        Self {
            state: AtomicUsize::new(0),
        }
    }

    /// indicative that the giving signal is now set.
    #[inline]
    pub fn wake_up(&self) -> bool {
        self.state
            .compare_exchange(SET, UNSET, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
    }

    /// indicative that the giving signal is now asleep.
    #[inline]
    pub fn make_asleep(&self) -> bool {
        self.state
            .compare_exchange(SLEEPY, SLEEPING, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
    }

    /// indicative that the giving signal is now sleepy.
    #[inline]
    pub fn make_sleepy(&self) -> bool {
        self.state
            .compare_exchange(UNSET, SLEEPY, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
    }

    #[inline]
    pub fn become_paused(&self) -> bool {
        self.state
            .compare_exchange(SLEEPING, SET, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
    }

    /// `probe_sleepy` returns true/false if owner is now sleepy.
    #[inline]
    pub fn probe_sleepy(&self) -> bool {
        self.state.load(Ordering::Acquire) == SLEEPY
    }

    /// `probe_asleep` returns true/false if owner is now asleep.
    #[inline]
    pub fn probe_asleep(&self) -> bool {
        self.state.load(Ordering::Acquire) == SLEEPING
    }

    /// `probe_paused` returns true/false if owner is now idle.
    #[inline]
    pub fn probe_paused(&self) -> bool {
        self.state.load(Ordering::Acquire) == SET
    }
}
