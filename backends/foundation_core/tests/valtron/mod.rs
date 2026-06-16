#[cfg(not(target_arch = "wasm32"))]
mod channel_backpressure_tests;
mod flatten_combinators;
mod futures_in_valtron;
mod map_circuit;
#[cfg(not(target_arch = "wasm32"))]
mod notification_based_waiting;
#[cfg(not(target_arch = "wasm32"))]
mod shutdown_mechanism_tests;
#[cfg(not(target_arch = "wasm32"))]
mod spread;
mod stream_future;
mod stream_iterators;
mod sync_boundary_helpers;
mod task_iterators;
#[cfg(not(target_arch = "wasm32"))]
mod thread_yielder_tests;
#[cfg(not(target_arch = "wasm32"))]
mod threaded_future;
mod units;
mod wasm_js_yield_integration;
mod wasm_yielder_tests;
mod valtron_macro_tests;
#[cfg(feature="multi")]
mod multi_workers;
