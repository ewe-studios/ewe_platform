//! The panic boundary of the wasm host entries (issue #742).
//!
//! WHY: `wasm32-unknown-unknown` builds with `panic = "abort"`. A panic in a
//! handler traps the module: the trap becomes a JavaScript exception and the
//! caller never receives a response. A production page or worker has no test
//! harness to catch that exception, so one failing request takes the whole
//! module down. [`catch_panic`] runs the request future inside a JavaScript
//! `try`/`catch`, so a trap becomes a value the bridge can answer with 500.
//!
//! WHAT: [`catch_panic`] drives any future and reports a panic as
//! [`PanicCaught`]. Each poll crosses one guarded host call.
//!
//! HOW: the guard is a JavaScript function, imported with `inline_js`. It calls
//! a Rust callback inside `try`. The callback polls the wrapped future. A trap
//! discards the wasm frames below the callback, the JavaScript `catch` returns
//! `false`, and the Rust frame above the call resumes normally. The module stays
//! usable because the engine restores the wasm stack on a trap.
//!
//! A trap does not run the destructors of the discarded frames. A future that
//! trapped mid-poll can hold values whose ownership bookkeeping is now
//! inconsistent: dropping it corrupts the allocator, and a later allocation
//! then traps with "memory access out of bounds". The guard therefore leaks the
//! future after a trap. One panicking request leaks its future and the
//! allocations of its discarded frames. That is the cost of serving again with
//! `panic = "abort"`; a panic is a bug, not a normal path. Log the trap at the
//! host and fix the handler.

use core::future::poll_fn;
use core::future::Future;
use core::pin::Pin;
use core::task::Poll;

use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = r#"
/**
 * Runs `callback` inside a JavaScript `try`.
 *
 * A Rust panic traps the wasm module. The trap is thrown as a JavaScript
 * exception at the call below, so `catch` sees it. Returns `true` when the
 * callback returned normally and `false` when it trapped.
 */
export function guard_poll(callback) {
    try {
        callback();
        return true;
    } catch (error) {
        console.error("foundation_http: a handler panicked:", error);
        return false;
    }
}
"#)]
extern "C" {
    /// Run `callback` inside a JavaScript `try`. `false` means it trapped.
    #[wasm_bindgen(js_name = guard_poll)]
    fn guard_poll(callback: &mut dyn FnMut()) -> bool;
}

/// The wrapped future panicked. The module is still usable; the request is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanicCaught;

impl core::fmt::Display for PanicCaught {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("a handler panicked")
    }
}

impl core::error::Error for PanicCaught {}

/// Drive `future` to completion, catching a panic in any poll.
///
/// # Errors
///
/// `Err(PanicCaught)` when the future panics. The caller answers 500 and keeps
/// serving other requests. The future is leaked, not dropped; see the module
/// documentation for the reason.
///
/// # Panics
///
/// A panic of the wrapped future never escapes; it becomes `Err`. A panic in
/// this function itself is not caught.
pub async fn catch_panic<F: Future>(future: F) -> Result<F::Output, PanicCaught> {
    // A `Box` gives the guard a stable address and lets it leak the future.
    let mut future: Option<Pin<Box<F>>> = Some(Box::pin(future));
    poll_fn(|cx| {
        let mut polled = None;
        let completed = {
            let pinned = future.as_mut().expect("the future is live before a poll");
            guard_poll(&mut || polled = Some(pinned.as_mut().poll(cx)))
        };
        match (completed, polled) {
            (true, Some(Poll::Ready(value))) => Poll::Ready(Ok(value)),
            (true, Some(Poll::Pending)) => Poll::Pending,
            // A trap, or a host that never ran the callback.
            _ => {
                if let Some(boxed) = future.take() {
                    core::mem::forget(boxed);
                }
                Poll::Ready(Err(PanicCaught))
            }
        }
    })
    .await
}
