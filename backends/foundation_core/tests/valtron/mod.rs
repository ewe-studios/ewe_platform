// Dual-target: runs natively and on wasm.
mod async_first_api;
#[cfg(not(target_family = "wasm"))]
mod bridge_park;
mod bridge_park_executor;
mod park_busy_loop;
mod local_executor;
mod wake_registry;
#[cfg(not(target_family = "wasm"))]
mod broadcast_tests;
mod builder_tests;
#[cfg(not(target_family = "wasm"))]
mod channel_backpressure_tests;
mod flatten_combinators;
mod cancellable_future;
#[cfg(feature = "std")]
mod ctx;
#[cfg(not(target_family = "wasm"))]
mod cross_runtime;
mod futures_in_valtron;
mod map_circuit;
#[cfg(not(target_family = "wasm"))]
mod notification_based_waiting;
#[cfg(not(target_family = "wasm"))]
mod shutdown_mechanism_tests;
mod sleep_tests;
#[cfg(not(target_family = "wasm"))]
mod spread;
#[cfg(not(target_family = "wasm"))]
mod spread_drain;
#[cfg(feature = "std")]
// Blocking sync-boundary helpers do not exist on JS-hosted wasm.
#[cfg(not(all(target_family = "wasm", any(feature = "js-wasmbindgen", feature = "js-foundation-wasm"))))]
mod state_task;
mod stream_future;
mod stream_iterators;
// Blocking sync-boundary helpers do not exist on JS-hosted wasm.
#[cfg(not(all(target_family = "wasm", any(feature = "js-wasmbindgen", feature = "js-foundation-wasm"))))]
mod sync_boundary_helpers;
mod task_iterators;
#[cfg(not(target_family = "wasm"))]
mod thread_yielder_tests;
#[cfg(not(target_family = "wasm"))]
mod threaded_future;
mod threaded_future_std;
mod units;
mod wasm_js_yield_integration;
mod wasm_yielder_tests;
// Blocking sync-boundary helpers do not exist on JS-hosted wasm.
#[cfg(not(all(target_family = "wasm", any(feature = "js-wasmbindgen", feature = "js-foundation-wasm"))))]
mod valtron_macro_tests;
#[cfg(not(target_family = "wasm"))]
mod wakeable_stream;
#[cfg(not(target_family = "wasm"))]
mod waker_queue_bridge;
#[cfg(not(target_family = "wasm"))]
mod pipe_primitive;
#[cfg(not(target_family = "wasm"))]
mod pipe_mapped_tests;
#[cfg(feature="multi")]
mod multi_workers;
#[cfg(not(target_family = "wasm"))]
mod background_jobs;
#[cfg(not(target_family = "wasm"))]
mod delayed_iterators;
mod drain;
mod multi_allocatable_threads;
#[cfg(not(target_family = "wasm"))]
mod single_executor;
mod task_status;
mod wrappers;
