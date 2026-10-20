//! wasm-bindgen clock: `Performance.now()` and `Date.now()` through wasm-bindgen glue.

use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};

#[wasm_bindgen]
extern "C" {
    /// Type for the [global object](https://developer.mozilla.org/en-US/docs/Glossary/Global_object).
    type Global;

    /// Returns the [`Performance`](https://developer.mozilla.org/en-US/docs/Web/API/Performance) object.
    #[wasm_bindgen(method, getter)]
    fn performance(this: &Global) -> JsValue;

    /// Type for the [`Performance` object](https://developer.mozilla.org/en-US/docs/Web/API/Performance).
    type Performance;

    /// Binding to [`Performance.now()`](https://developer.mozilla.org/en-US/docs/Web/API/Performance/now).
    #[wasm_bindgen(method)]
    fn now(this: &Performance) -> f64;

    /// Binding to [`Performance.timeOrigin`](https://developer.mozilla.org/en-US/docs/Web/API/Performance/timeOrigin).
    #[cfg(target_feature = "atomics")]
    #[wasm_bindgen(method, getter, js_name = timeOrigin)]
    fn time_origin(this: &Performance) -> f64;
}

thread_local! {
    static PERFORMANCE: Performance = {
        let global: Global = js_sys::global().unchecked_into();
        let performance = global.performance();

        if performance.is_undefined() {
            panic!("`Performance` object not found")
        } else {
            performance.unchecked_into()
        }
    };
}

#[cfg(target_feature = "atomics")]
thread_local! {
    static ORIGIN: f64 = PERFORMANCE.with(Performance::time_origin);
}

/// Monotonic milliseconds for [`super::Instant`]: `performance.now()`, plus
/// `performance.timeOrigin` on threaded builds so instants agree across workers.
///
/// # Panics
///
/// Panics if the `Performance` object is not available (e.g. in a worklet).
pub(super) fn monotonic_now_ms() -> f64 {
    PERFORMANCE.with(|performance| {
        #[cfg(target_feature = "atomics")]
        return ORIGIN.with(|origin| performance.now() + origin);

        #[cfg(not(target_feature = "atomics"))]
        performance.now()
    })
}

/// Wall-clock milliseconds since the Unix epoch for [`super::SystemTime`]: `Date.now()`.
pub(super) fn wall_clock_ms() -> f64 {
    js_sys::Date::now()
}
