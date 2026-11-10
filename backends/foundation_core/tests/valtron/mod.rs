// Dual-target: runs natively and on wasm.
mod async_first_api;
#[cfg(not(target_family = "wasm"))]
mod bridge_park;
mod bridge_park_executor;
mod bridged_waker_liveness;
#[cfg(not(target_family = "wasm"))]
mod broadcast_tests;
#[cfg(not(target_family = "wasm"))]
mod builder_panic_handler;
mod builder_tests;
mod cancellable_future;
#[cfg(not(target_family = "wasm"))]
mod channel_backpressure_tests;
mod clock_driven_tasks;
mod clock_injection;
mod clock_wall;
#[cfg(not(target_family = "wasm"))]
mod cross_runtime;
mod ctx_reexport;
#[cfg(feature = "std")]
#[cfg(not(target_family = "wasm"))]
mod event_driven_queue_readiness;
mod flatten_combinators;
mod futures_in_valtron;
mod local_executor;
mod map_circuit;
#[cfg(not(target_family = "wasm"))]
mod notification_based_waiting;
mod park_busy_loop;
mod pool_seed_root;
mod readiness_deadline_tests;
mod readiness_register_race;
#[cfg(not(target_family = "wasm"))]
mod shutdown_mechanism_tests;
mod sleep_tests;
#[cfg(not(target_family = "wasm"))]
mod spread;
#[cfg(not(target_family = "wasm"))]
mod spread_drain;
#[cfg(feature = "std")]
// Blocking sync-boundary helpers do not exist on JS-hosted wasm.
#[cfg(not(all(
    target_family = "wasm",
    any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")
)))]
mod state_task;
#[cfg(not(target_family = "wasm"))]
mod stream_future;
mod stream_iterators;
#[cfg(not(target_family = "wasm"))]
mod targeted_wake;
mod wake_registry;
// Blocking sync-boundary helpers do not exist on JS-hosted wasm.
#[cfg(not(all(
    target_family = "wasm",
    any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")
)))]
mod sync_boundary_helpers;
mod task_iterators;
#[cfg(not(target_family = "wasm"))]
mod thread_yielder_tests;
#[cfg(not(target_family = "wasm"))]
mod threaded_future;
mod threaded_future_std;
mod units;
mod wasm_js_yield_integration;
// Shared scenarios for the wasm runtime suites (also included by
// `integration/wasm_tests`); a JS-hosted wasm build only.
#[cfg(all(
    target_family = "wasm",
    any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")
))]
mod wasm_wake_support;
mod wasm_yielder_tests;
// Blocking sync-boundary helpers do not exist on JS-hosted wasm.
#[cfg(not(target_family = "wasm"))]
mod background_jobs;
#[cfg(not(target_family = "wasm"))]
mod background_shutdown_deadline;
#[cfg(not(feature = "multi"))]
#[cfg(not(target_family = "wasm"))]
mod callback_waker_thread_confinement;
#[cfg(not(target_family = "wasm"))]
mod delayed_iterators;
mod drain;
#[cfg(not(target_family = "wasm"))]
mod driven_stream_async_wake;
mod limits_config_tests;
mod multi_allocatable_threads;
#[cfg(feature = "multi")]
mod multi_workers;
#[cfg(not(feature = "multi"))]
#[cfg(not(target_family = "wasm"))]
mod non_sendable_split_observer_wake;
#[cfg(not(target_family = "wasm"))]
mod pipe_closed_readiness;
#[cfg(not(target_family = "wasm"))]
mod pipe_mapped_tests;
#[cfg(not(target_family = "wasm"))]
mod pipe_primitive;
#[cfg(feature = "multi")]
mod pool_config_tests;
#[cfg(not(target_family = "wasm"))]
mod self_wake_fairness;
#[cfg(feature = "multi")]
mod shutdown_single_deadline;
#[cfg(not(target_family = "wasm"))]
mod single_executor;
#[cfg(feature = "multi")]
mod slow_poll_warning;
#[cfg(not(target_family = "wasm"))]
mod split_observer_wake;
mod task_status;
#[cfg(not(all(
    target_family = "wasm",
    any(feature = "js-wasmbindgen", feature = "js-foundation-wasm")
)))]
mod valtron_macro_tests;
#[cfg(not(target_family = "wasm"))]
mod wakeable_stream;
#[cfg(not(target_family = "wasm"))]
mod waker_identity;
#[cfg(not(target_family = "wasm"))]
mod waker_queue_bridge;
#[cfg(feature = "multi")]
mod worker_panic_accounting;
mod wrappers;
