//! The writable side of `js_stream`: a waiting source must park, not spin.
//!
//! WHY: the pull of a `ReadableStream` used to call the source again until it
//! had bytes. On one wasm thread that never yields, so a timer or a producer
//! never runs (#561). The source now answers `Pending`, and the pull returns a
//! promise that the readiness or a `setTimeout(0)` resolves.
//! HOW: a source has no data until a host timer fires. The test reads the
//! stream once; the read resolves only when the timer ran and the next pull
//! produced its chunk.

#![cfg(all(
    target_family = "wasm",
    any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")
))]

use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use foundation_core::valtron::js_stream::{iterator_to_readable_stream, JsStreamItem};
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::wasm_bindgen_test;

/// A source with no data until a host timer sets `ready`.
struct TimedSource {
    ready: Arc<AtomicBool>,
    timer_started: Rc<Cell<bool>>,
    sent: bool,
}

impl Iterator for TimedSource {
    type Item = JsStreamItem;

    fn next(&mut self) -> Option<Self::Item> {
        if self.sent {
            return None;
        }
        if self.ready.load(Ordering::SeqCst) {
            self.sent = true;
            return Some(JsStreamItem::Chunk(b"hello".to_vec()));
        }
        if !self.timer_started.get() {
            self.timer_started.set(true);
            let ready = Arc::clone(&self.ready);
            schedule(move || ready.store(true, Ordering::SeqCst));
        }
        Some(JsStreamItem::Pending)
    }
}

/// Run `callback` on the next macrotask.
fn schedule(callback: impl FnOnce() + 'static) {
    let callback = wasm_bindgen::closure::Closure::once_into_js(callback);
    let global = js_sys::global();
    let set_timeout = js_sys::Reflect::get(&global, &JsValue::from_str("setTimeout"))
        .expect("setTimeout")
        .dyn_into::<js_sys::Function>()
        .expect("setTimeout is a function");
    let _ = set_timeout.call2(&global, callback.unchecked_ref(), &JsValue::from_f64(0.0));
}

#[wasm_bindgen_test]
async fn a_waiting_source_parks_until_the_host_timer_runs() {
    let source = TimedSource {
        ready: Arc::new(AtomicBool::new(false)),
        timer_started: Rc::new(Cell::new(false)),
        sent: false,
    };
    let stream = iterator_to_readable_stream(Box::new(source), None).expect("ReadableStream");
    let reader: web_sys::ReadableStreamDefaultReader = stream
        .get_reader()
        .unchecked_into::<web_sys::ReadableStreamDefaultReader>(
    );

    let mut received = Vec::new();
    for _ in 0..10 {
        let result = wasm_bindgen_futures::JsFuture::from(reader.read())
            .await
            .expect("read");
        let done = js_sys::Reflect::get(&result, &JsValue::from_str("done"))
            .ok()
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        if done {
            break;
        }
        let value = js_sys::Reflect::get(&result, &JsValue::from_str("value")).expect("value");
        received.extend_from_slice(&js_sys::Uint8Array::new(&value).to_vec());
    }
    assert_eq!(
        received, b"hello",
        "the timer ran and the next pull produced the chunk"
    );
}
