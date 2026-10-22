//! Scheduling without a running pool is an error, not a panic (`multi`).
//!
//! WHY a separate test binary: the pool is process-global, and other tests
//! start one; here nothing does.

#![cfg(all(feature = "multi", not(target_family = "wasm")))]

use foundation_core::valtron::{execute, send, Ctx, TaskIterator, TaskStatus};

struct Once(bool);

impl TaskIterator for Once {
    type Ready = u8;
    type Pending = ();
    fn next_status(&mut self, _ctx: Ctx) -> Option<TaskStatus<u8, ()>> {
        if self.0 {
            return None;
        }
        self.0 = true;
        Some(TaskStatus::Ready(1))
    }
}

#[test]
fn send_and_execute_without_a_pool_return_an_error() {
    assert!(send(Once(false)).is_err(), "send without a pool");
    assert!(execute(Once(false), None).is_err(), "execute without a pool");
}
