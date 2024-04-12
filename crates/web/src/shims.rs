use anyhow::{anyhow, Result};
use cfg_if::cfg_if;
use futures;
use web_sys::{Document, Window};

pub fn set_panic_hook() {
    // When the `console_error_panic_hook` feature is enabled, we can call the
    // `set_panic_hook` function at least once during initialization, and then
    // we will get better error messages if our code ever panics.
    //
    // For more details see
    // https://github.com/rustwasm/console_error_panic_hook#readme
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

pub fn window() -> Result<Window> {
    web_sys::window().ok_or_else(|| anyhow!("No Window Found"))
}

pub fn document() -> Result<Document> {
    window()?
        .document()
        .ok_or_else(|| anyhow!("No Document Found"))
}

/// Spawns and runs a thread-local [`Future`] in a platform-independent way.
///
/// This can be used to interface with any `async` code by spawning a task
/// to run a `Future`.
///
/// ## Limitations
///
/// When in WASM: this uses Promises underneath and hence will be async.
///
/// When in test: this blocks current thread until the future completes.
///
/// When in normal runtime: this blocks current thread and schedules
/// future on current thrad for completion until the future completion,
/// which means if you call it in a different thread then it runs the
/// future in that thread till completion.
pub fn spawn_local<F>(future: F)
where
    F: futures::Future<Output = ()> + 'static,
{
    cfg_if! {
        if #[cfg(target_arch="wasm32")] {
            wasm_bindgen_futures::spawn_local(future);
        } else if #[cfg(any(test, doctest))] {
            tokio_test::block_on(future);
        } else if #[cfg(feature ="server")] {
            tokio::task::spawn_local(async move {
                future.await;
            })
        } else {
            futures::executor::block_on(future)
        }
    }
}
