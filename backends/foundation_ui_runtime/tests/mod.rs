#[cfg(not(target_family = "wasm"))]
mod action_flow_tests;
#[cfg(not(target_family = "wasm"))]
mod action_macro_fail_tests;
#[cfg(not(target_family = "wasm"))]
mod action_macro_tests;
mod action_manager_tests;
mod action_receiver_tests;
mod action_registry_tests;
#[cfg(not(target_family = "wasm"))]
mod dispatch_delivery_tests;
#[cfg(not(target_family = "wasm"))]
mod dispatch_errors_tests;
#[cfg(not(target_family = "wasm"))]
mod dispatch_fuzz_tests;
#[cfg(not(target_family = "wasm"))]
mod dispatch_hello_tests;
#[cfg(not(target_family = "wasm"))]
mod dispatch_local_tests;
#[cfg(not(target_family = "wasm"))]
mod dispatch_support;
#[cfg(not(target_family = "wasm"))]
mod dispatch_tests;
#[cfg(not(target_family = "wasm"))]
mod network_app_tests;
mod placement_order_tests;
#[cfg(not(target_family = "wasm"))]
mod protected_route_tests;
mod signer_context_tests;
mod state_keys_tests;
mod state_signer_tests;
mod support;
#[cfg(not(target_family = "wasm"))]
mod view_fail_tests;
mod view_lower_tests;
#[cfg(not(target_family = "wasm"))]
mod view_macro_tests;
