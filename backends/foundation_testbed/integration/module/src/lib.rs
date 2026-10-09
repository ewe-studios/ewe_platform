//! Sample `#[wasm_test]` crate (feature 13): every execution-model path in one
//! module — sync pass, assertion failure (panic-hook report + trap), ignore,
//! should_panic (runner inverts), and an async case driven by the owned
//! `schedule_timeout` re-poll loop. The JS harness asserts the `host_report`
//! stream these produce.
//!
//! `std` cdylib (default allocator + panic handler); the library crates stay `no_std`.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use foundation_macros::wasm_test;

/// A future that is `Pending` exactly once — forces one trip through the owned
/// JS-yield loop (`schedule_timeout` re-poll) before resolving.
struct YieldOnce(bool);

impl Future for YieldOnce {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        if self.0 {
            Poll::Ready(())
        } else {
            self.0 = true;
            Poll::Pending
        }
    }
}

#[wasm_test]
fn passes_simple() {
    assert_eq!(2 + 2, 4);
}

#[wasm_test]
fn fails_with_assertion() {
    assert_eq!(1 + 1, 3, "math is broken on purpose");
}

#[wasm_test(ignore)]
fn ignored_case() {
    unreachable!("ignored cases never run");
}

#[wasm_test(should_panic)]
fn panics_as_expected() {
    panic!("this panic is the expected outcome");
}

#[wasm_test]
async fn async_completes_after_yield() {
    YieldOnce(false).await;
    assert_eq!(6 * 7, 42);
}
