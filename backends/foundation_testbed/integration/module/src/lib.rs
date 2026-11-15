//! Sample `#[wasm_test]` crate (feature 13): every execution-model path in one
//! module — sync pass, assertion failure (panic-hook report + trap), ignore,
//! should_panic (runner inverts), an async case driven by the owned
//! `schedule_timeout` re-poll loop, and the local-endpoint case of spec 88.
//! The JS harness asserts the `host_report` stream these produce.
//!
//! `std` cdylib (default allocator + panic handler); the library crates stay `no_std`.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use foundation_macros::wasm_test;
use foundation_wasm::abi::web::{invoke_as_f64, register_function};

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

/// Start one fetch of the host's `/echo` route, and record its verdict.
///
/// The verdict is `0` while the fetch is pending, `1` when the answer has the
/// method, the headers and the body the case sent, and a negative number on
/// failure. A case reads it after a yield, so the fetch can progress.
const START_ECHO: &str = r#"
function () {
  const base = globalThis.__FWT_BASE_URL__;
  if (!base) {
    globalThis.__fwt_echo_state = -4;
    return -4;
  }
  globalThis.__fwt_echo_state = 0;
  fetch(base + "/echo", {
    method: "POST",
    headers: { "x-fwt-probe": "owned" },
    body: "hello",
  })
    .then(async (response) => {
      const text = await response.text();
      const ok =
        response.status === 200 &&
        text.includes('"method":"POST"') &&
        text.includes('"path":"/echo"') &&
        text.includes('"x-fwt-probe":"owned"') &&
        text.includes('"body":"aGVsbG8="') &&
        text.includes('"bytes":5');
      globalThis.__fwt_echo_state = ok ? 1 : -1;
    })
    .catch(() => {
      globalThis.__fwt_echo_state = -3;
    });
  return 0;
}
"#;

/// The current verdict of [`START_ECHO`].
const ECHO_STATE: &str = "function () { return globalThis.__fwt_echo_state ?? -5; }";

#[wasm_test]
async fn echo_endpoint_answers() {
    let start = register_function(START_ECHO);
    assert_eq!(invoke_as_f64(start.handler, &[]), 0.0, "the fetch starts");

    let state = register_function(ECHO_STATE);
    let mut verdict = 0.0;
    for _ in 0..200 {
        YieldOnce(false).await;
        verdict = invoke_as_f64(state.handler, &[]);
        if verdict != 0.0 {
            break;
        }
    }
    assert_eq!(
        verdict, 1.0,
        "the host answered 200 with the method, headers and body the case sent (verdict {verdict})"
    );
}
