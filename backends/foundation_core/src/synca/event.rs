// Implements an Lock notification primitive usable in threads.

#[cfg(not(feature = "web_spin_lock"))]
use std::sync::{Condvar, Mutex};

#[cfg(feature = "web_spin_lock")]
use wasm_sync::{CondVar, Mutex};

use super::Waker;

/// LockState defines the underlying state of a Condvar based
/// locks which will allow us sleep a thread silently without eating up
/// CPU cycles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LockState {
    Free,
    Locked,
    Released,
}

/// LockSignal allows us sleep a thread or process until a signal
/// gets delivered via it's underlying CondVar.
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

pub(crate) enum NotifyDirective {
    One,
    All,
}

impl Waker for LockSignal {
    fn wake(&self) {
        self.signal_all();
    }
}

impl LockSignal {
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
        };
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

    pub fn try_lock(&self) -> bool {
        let mut current_state = self.lock.lock().unwrap();
        if *current_state == LockState::Locked {
            return false;
        }
        *current_state = LockState::Locked;
        drop(current_state);
        return true;
    }

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

    pub fn wait(&self) {
        let mut current_state = self.lock.lock().unwrap();

        // if its in the free state then no wait is required;
        if *current_state == LockState::Free {
            return;
        }

        // wait till its back in the locked state
        loop {
            if *current_state == LockState::Released {
                // set back to free state
                *current_state = LockState::Free;
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

        let (sender, reciever) = mpsc::channel::<()>();

        let latch_clone = latch.clone();
        let handler = thread::spawn(move || loop {
            latch_clone.try_lock();
            sender.send(()).expect("should send");
            latch_clone.wait();
            break;
        });

        thread::sleep(Duration::from_millis(100));

        let _ = reciever.recv();
        assert_eq!(LockState::Locked, latch.probe());

        latch.signal_all();
        handler.join().expect("should safely join");

        assert_eq!(LockState::Free, latch.probe());
    }
}
