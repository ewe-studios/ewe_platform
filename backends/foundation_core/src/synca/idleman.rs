// Implements idle management handler for handling idle checks.

use std::{
    sync::atomic::{self, AtomicU32, Ordering},
    time,
};

use crate::retries::{ExponentialBackoffDecider, RetryDecider, RetryState};

#[derive(Clone, Debug)]
pub enum SleepyState {
    Init,
    Ongoing(RetryState),
    Expired,
}

/// [`SleepyMan`] provides a sleep management module
/// that allows you query for the next sleep duration
/// to a maximum value since both duration wise and
/// sleep count.
///
/// This allows us continuously request the next sleeping
/// duration until a maximum count of sleep as been given
/// at which point no more sleep duration will be provided.
///
/// It provides a nice way to manage the sleeping allowances
/// for a process by encapsulating the idea of rented sleep
/// managed till an allowable period.
pub struct SleepyMan {
    max_sleeps: u32,
    last_state: SleepyState,
    retry_decider: ExponentialBackoffDecider,
}

impl SleepyMan {
    pub fn new(max_idles: u32, retry_decider: ExponentialBackoffDecider) -> Self {
        Self {
            max_sleeps: max_idles,
            retry_decider,
            last_state: SleepyState::Init,
        }
    }

    pub fn state(&self) -> SleepyState {
        self.last_state.clone()
    }

    pub fn reset(&mut self) {
        self.last_state = SleepyState::Init;
    }

    pub fn next_sleep(&mut self) -> SleepyState {
        match &self.last_state {
            SleepyState::Init => {
                self.last_state = SleepyState::Ongoing(RetryState::new(0, self.max_sleeps, None));
                self.last_state.clone()
            }
            SleepyState::Ongoing(retry_state) => {
                self.last_state = match self.retry_decider.decide(retry_state.clone()) {
                    Some(inner) => SleepyState::Ongoing(inner.clone()),
                    None => SleepyState::Expired,
                };

                self.last_state.clone()
            }
            SleepyState::Expired => SleepyState::Expired,
        }
    }
}

/// [`IdleMan`] provides an idle counter whose job is pretty simple
/// when giving a max idle count, each time the counter is incremented
/// you are returned a potential sleep duration that you should consider
/// sleeping for since you are idle.
///
/// Based on setup the increment of each increment of the idle count to the
/// max will return a constant sleep duration wrapped in an Option, which means
/// it can be `None` or `Some(time::Duration)` which allows you to either
/// indicate the caller should sleep (sleep or stay dormant) for some time
/// before attempting their next operation.
pub struct IdleMan {
    sleepy: SleepyMan,
    counter: AtomicU32,
    base_sleep: Option<time::Duration>,
    max_idle: u32,
}

static DEFAULT_BASIC_SLEEP: time::Duration = time::Duration::from_millis(10);
static DEFAULT_MAX_SLEEP_COUNT: u32 = 5;

impl IdleMan {
    pub fn new(max_idle: u32, base_sleep: Option<time::Duration>, sleepy: SleepyMan) -> Self {
        Self {
            sleepy,
            max_idle,
            base_sleep,
            counter: AtomicU32::new(0),
        }
    }

    #[must_use] 
    pub fn basic(max_idle: u32) -> Self {
        Self::new(
            max_idle,
            Some(DEFAULT_BASIC_SLEEP),
            SleepyMan::new(
                DEFAULT_MAX_SLEEP_COUNT,
                ExponentialBackoffDecider::default(),
            ),
        )
    }

    pub fn increment(&mut self) -> Option<time::Duration> {
        let current_count = self.counter.load(atomic::Ordering::SeqCst);

        // start calling sleepy man for sleep duration
        if current_count >= self.max_idle {
            return match self.sleepy.next_sleep() {
                SleepyState::Ongoing(retry_state) => match retry_state.wait {
                    None => self.base_sleep,
                    Some(inner) => Some(inner),
                },
                SleepyState::Expired => {
                    assert!(self.try_reset(), "Failed to reset counter");
                    self.sleepy.reset();
                    self.base_sleep
                }
                SleepyState::Init => unreachable!(),
            };
        }

        self.counter.fetch_add(1, Ordering::SeqCst);
        self.base_sleep
    }

    /// count returns the current count number of the idle counter.
    pub fn count(&self) -> u32 {
        self.counter.load(atomic::Ordering::Acquire)
    }

    /// `try_reset` attempts to reset the counter if it's
    /// reached the maximum allowed amount and returns true
    /// else returns false.
    pub fn try_reset(&self) -> bool {
        self.counter
            .compare_exchange(
                self.max_idle,
                0,
                atomic::Ordering::SeqCst,
                atomic::Ordering::Acquire,
            )
            .is_ok()
    }
}

#[cfg(test)]
mod test_idleman {
    use super::*;

    #[test]
    fn can_increment_for_max_allowed_idleness() {
        let sl = SleepyMan::new(3, ExponentialBackoffDecider::default());
        let mut idle = IdleMan::new(3, None, sl);

        assert_eq!(idle.increment(), None);
        assert_eq!(idle.increment(), None);
        assert_eq!(idle.increment(), None);
        assert_eq!(idle.increment(), None);
    }

    #[test]
    fn increment_to_max_leads_to_sleep_duration() {
        let sl = SleepyMan::new(3, ExponentialBackoffDecider::default());
        let mut idle = IdleMan::new(3, None, sl);

        assert!(idle.increment().is_none());
        assert!(idle.increment().is_none());
        assert!(idle.increment().is_none());
        assert!(idle.increment().is_none());

        assert!(idle.increment().is_some());
        assert!(idle.increment().is_some());
        assert!(idle.increment().is_some());

        // end of random sleep
        assert!(idle.increment().is_none());
    }

    #[test]
    fn increment_to_max_resets_sleeper() {
        let dfs = Some(time::Duration::from_millis(100));
        let sl = SleepyMan::new(3, ExponentialBackoffDecider::default());
        let mut idle = IdleMan::new(3, dfs, sl);

        assert_eq!(idle.increment(), dfs);
        assert_eq!(idle.increment(), dfs);
        assert_eq!(idle.increment(), dfs);
        assert_eq!(idle.increment(), dfs);

        assert!(idle.increment().is_some());
        assert!(idle.increment().is_some());
        assert!(idle.increment().is_some());

        assert_eq!(idle.increment(), dfs);
        assert_eq!(idle.increment(), dfs);
        assert_eq!(idle.increment(), dfs);
        assert_eq!(idle.increment(), dfs);

        assert!(idle.increment().is_some());
        assert!(idle.increment().is_some());
        assert!(idle.increment().is_some());
        assert!(idle.increment().is_some());
    }
}
