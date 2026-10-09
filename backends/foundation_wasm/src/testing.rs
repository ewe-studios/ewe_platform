//! WHY: Feature 13 — a fully owned wasm test-execution model (no wasm-bindgen).
//! `#[wasm_test]` (foundation_macros) compiles cases into `__fwt_*` exports; the
//! outcome must flow back to the JS runner over OUR ABI.
//!
//! WHAT: The `no_std` support half of the model: case bookkeeping
//! ([`enter_case`]/[`current_case`]), the result reports ([`pass`]/[`fail`]/
//! [`fail_current`]/[`ignored`]) riding the `host_report` import, and the async
//! driver ([`run_async`]) that re-polls a case future through the owned
//! `schedule_timeout` loop (the JS-yield path).
//!
//! HOW: wasm32 panics abort (no unwinding), so panic capture is split: the
//! `#[wasm_test]` expansion installs a `std` panic hook in the USER crate (this
//! crate is `no_std`) that calls [`fail_current`] with the panic text BEFORE the
//! trap; the JS runner catches the trap itself and, for `should_panic` cases,
//! inverts the verdict. Statuses: 0 = pass, 1 = fail, 2 = ignored.

use alloc::boxed::Box;
use alloc::string::{String, ToString};

use foundation_nostd::comp::basic::Mutex;

use crate::abi;

/// Case outcome statuses carried by `host_report` (shared with the JS runner).
pub const STATUS_PASS: u32 = 0;
/// Failure (assertion or panic).
pub const STATUS_FAIL: u32 = 1;
/// Skipped (`#[wasm_test(ignore)]`).
pub const STATUS_IGNORED: u32 = 2;

/// The case currently executing — the panic hook reads it to attribute a panic
/// message to the right case (one case runs at a time; the runner is sequential).
static CURRENT_CASE: Mutex<Option<String>> = Mutex::new(None);

/// Whether the current case already reported (a panic hook report must not be
/// followed by a pass report, and vice versa).
static REPORTED: Mutex<bool> = Mutex::new(false);

/// Mark `name` as the executing case (called at the top of every `__fwt_` export).
pub fn enter_case(name: &str) {
    *CURRENT_CASE
        .lock()
        .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner) =
        Some(String::from(name));
    *REPORTED
        .lock()
        .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner) = false;
}

/// The executing case's name, if any (panic-hook attribution).
pub fn current_case() -> Option<String> {
    CURRENT_CASE
        .lock()
        .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
        .clone()
}

fn mark_reported() -> bool {
    let mut guard = REPORTED
        .lock()
        .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
    let already = *guard;
    *guard = true;
    already
}

/// Ship a raw report over the ABI. The message bytes are borrowed for the duration
/// of the call only (JS copies them synchronously).
fn report(status: u32, message: &str) {
    if mark_reported() {
        return; // one verdict per case
    }
    let bytes = message.as_bytes();
    #[allow(unused_unsafe)]
    unsafe {
        abi::web::host_report(status, bytes.as_ptr() as u64, bytes.len() as u64);
    };
}

/// Report the named case as passed.
pub fn pass(name: &str) {
    report(STATUS_PASS, name);
}

/// Report the named case as failed with `message`.
pub fn fail(name: &str, message: &str) {
    let mut text = String::from(name);
    text.push_str(": ");
    text.push_str(message);
    report(STATUS_FAIL, &text);
}

/// Report the named case as ignored/skipped.
pub fn ignored(name: &str) {
    report(STATUS_IGNORED, name);
}

/// Report the CURRENT case as failed — the panic-hook entry point (the hook only
/// has the panic info; the case name comes from [`enter_case`]).
pub fn fail_current(message: &str) {
    let name = current_case().unwrap_or_else(|| String::from("<unknown case>"));
    fail(&name, message);
}

/// Drive an async case to completion on the owned JS-yield loop: poll once
/// synchronously; while `Pending`, re-poll via a zero-delay `schedule_timeout`.
/// Returns `0` when the case completed (and reported) synchronously, `1` when the
/// report will arrive asynchronously — the `__fwt_` export forwards this so the
/// runner knows whether to await.
pub fn run_async<F>(name: &str, future: F) -> u32
where
    F: core::future::Future<Output = ()> + 'static,
{
    use alloc::rc::Rc;
    use core::cell::RefCell;
    use core::pin::Pin;
    use core::task::{Context, Poll, Waker};

    type CaseFuture = Rc<RefCell<Pin<Box<dyn core::future::Future<Output = ()>>>>>;

    fn drive(name: String, future: CaseFuture) -> Poll<()> {
        let result = {
            let mut slot = future.borrow_mut();
            let waker = Waker::noop();
            let mut context = Context::from_waker(waker);
            slot.as_mut().poll(&mut context)
        };
        match result {
            Poll::Ready(()) => {
                pass(&name);
                Poll::Ready(())
            }
            Poll::Pending => {
                // Re-enter on the next host timer tick (the owned yield loop). The
                // schedule registry takes `Fn`, so the closure re-arms by cloning.
                #[cfg(target_family = "wasm")]
                {
                    let again = future.clone();
                    let case = name.clone();
                    abi::web::register_schedule(0.0, move || {
                        let _ = drive(case.clone(), again.clone());
                    });
                }
                Poll::Pending
            }
        }
    }

    let future: CaseFuture = Rc::new(RefCell::new(Box::pin(future)));
    match drive(name.to_string(), future) {
        Poll::Ready(()) => 0,
        Poll::Pending => 1,
    }
}
