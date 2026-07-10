#[cfg(not(target_family = "wasm"))]
mod broadcast_tests;
mod builder_tests;
#[cfg(not(target_family = "wasm"))]
mod channel_backpressure_tests;
mod flatten_combinators;
mod cancellable_future;
mod futures_in_valtron;
mod map_circuit;
#[cfg(not(target_family = "wasm"))]
mod notification_based_waiting;
#[cfg(not(target_family = "wasm"))]
mod shutdown_mechanism_tests;
#[cfg(not(target_family = "wasm"))]
mod spread;
mod stream_future;
mod stream_iterators;
mod sync_boundary_helpers;
mod task_iterators;
#[cfg(not(target_family = "wasm"))]
mod thread_yielder_tests;
#[cfg(not(target_family = "wasm"))]
mod threaded_future;
mod units;
mod wasm_js_yield_integration;
mod wasm_yielder_tests;
mod valtron_macro_tests;
#[cfg(not(target_family = "wasm"))]
mod waker_queue_bridge;
#[cfg(not(target_family = "wasm"))]
mod pipe_primitive;
#[cfg(not(target_family = "wasm"))]
mod pipe_mapped_tests;
#[cfg(feature="multi")]
mod multi_workers;
