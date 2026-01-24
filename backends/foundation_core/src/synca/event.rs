// Implements an Lock notification primitive usable in threads.

use std::sync::Mutex;
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::sync::Condvar;

use super::Waker;

/// `LockState` defines the underlying state of a Condvar based
/// locks which will allow us sleep a thread silently without eating up
/// CPU cycles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LockState {
    Free,
    Locked,
    Released,
}

/// `LockSignal` allows us sleep a thread or process until a signal
/// gets delivered via it's underlying `CondVar`.
pub struct LockSignal {
    /// The condition variable used to wait on an event,
    /// also provides a way to awake a sleeping thread.
    event: Condvar,

    /// The mutex used to protect the event.
    ///
    /// Inside is a state indicative of the current state of the
    /// the lock signal.
    lock: Mutex<LockState>,
}

// SAFETY: LockSignal is safe to use across unwind boundaries.
// The internal Mutex and Condvar are designed to maintain consistency
// even in the presence of panics, and LockState is a simple enum.
impl UnwindSafe for LockSignal {}
impl RefUnwindSafe for LockSignal {}

pub(crate) enum NotifyDirective {
    One,
    All,
}

impl Waker for LockSignal {
    fn wake(&self) {
        self.signal_all();
    }
}

impl Default for LockSignal {
    fn default() -> Self {
        Self::new()
    }
}

impl LockSignal {
    #[must_use]
    pub fn new() -> Self {
        Self {
            event: Condvar::new(),
            lock: Mutex::new(LockState::Free),
        }
    }

    pub(crate) fn signal(&self, directive: NotifyDirective) {
        let mut state = self.lock.lock().unwrap();
        *state = LockState::Released;
        drop(state);

        match directive {
            NotifyDirective::One => self.event.notify_one(),
            NotifyDirective::All => self.event.notify_all(),
        }
    }

    pub fn probe(&self) -> LockState {
        let current_state_guard = self.lock.lock().unwrap();
        let current_state = current_state_guard.clone();
        drop(current_state_guard);
        current_state
    }

    pub fn probe_locked(&self) -> bool {
        let current_state = self.lock.lock().unwrap();
        let is_locked = *current_state == LockState::Locked;
        drop(current_state);
        is_locked
    }

    /// Attempts to put the latch in a locked state thereby
    /// allowing to block the current thread readying
    /// you to call [`Self::wait`] after.
    ///
    /// But the latch might already be in a lock state
    /// and if so, its a no-op and in such cases we return
    /// false to communicate `LockSignal` is already locked
    /// but to be clear this does not mean an error, it just
    /// means something else locked this `LockSignal` and you
    /// are free to call `LockSignal::wait()` as well to wait
    /// for wakeup signal.
    pub fn try_lock(&self) -> bool {
        let mut current_state = self.lock.lock().unwrap();
        if *current_state == LockState::Locked {
            return false;
        }
        *current_state = LockState::Locked;
        drop(current_state);
        true
    }

    /// lock will just perform a lock operation on the
    /// `LockSignal` and internally handle when the lock is
    /// locked as a no-op operation.
    pub fn lock(&self) {
        let mut current_state = self.lock.lock().unwrap();
        if *current_state != LockState::Locked {
            *current_state = LockState::Locked;
        }
        drop(current_state);
    }

    pub fn signal_one(&self) {
        self.signal(NotifyDirective::One);
    }

    pub fn signal_all(&self) {
        self.signal(NotifyDirective::All);
    }

    /// [`lock_and_wait`] will lock the `LockSignal` and
    /// block the current thread till it gets a notification
    /// to wakeup.
    pub fn lock_and_wait(&self) {
        if self.try_lock() {
            tracing::debug!("LockSignal was locked");
        }
        self.wait();
    }

    /// wait blocks the current thread till the signal
    /// is received from the `CondVar`.
    pub fn wait(&self) {
        let mut current_state = self.lock.lock().unwrap();

        // if its in the free state then no wait is required;
        if *current_state == LockState::Free {
            tracing::debug!("Lock is free");
            return;
        }

        // wait till its back in the locked state
        tracing::debug!("Will loop till lock is free");
        loop {
            if *current_state == LockState::Released {
                // set back to free state
                *current_state = LockState::Free;
                tracing::debug!("Lock is now free");
                return;
            }

            // wait for the event to be signaled.
            current_state = self.event.wait(current_state).unwrap();
        }
    }
}

#[cfg(test)]
mod test_lock_signals {
    use std::{
        sync::{mpsc, Arc},
        thread,
        time::Duration,
    };

    use crate::synca::LockState;

    use super::LockSignal;

    #[test]
    fn can_lock_signal() {
        let latch = Arc::new(LockSignal::new());

        let (sender, receiver) = mpsc::channel::<()>();

        let latch_clone = latch.clone();
        let handler = thread::spawn(move || {
            latch_clone.try_lock();
            sender.send(()).expect("should send");
            latch_clone.wait();
        });

        thread::sleep(Duration::from_millis(100));

        let _ = receiver.recv();
        assert_eq!(LockState::Locked, latch.probe());

        latch.signal_all();
        handler.join().expect("should safely join");

        assert_eq!(LockState::Free, latch.probe());
    }
}
