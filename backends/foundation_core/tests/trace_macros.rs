//! `foundation_core`'s logging macros (`info!`, `debug!`, `warn!`, `error!`),
//! moved out of `src/trace/mod.rs` (tests live in `tests/`).
#![cfg(not(target_family = "wasm"))]

#[test]
fn logs_without_arg() {
    foundation_core::info!("Help me out");
    foundation_core::debug!("Help me out");
    foundation_core::warn!("Help me out");
    foundation_core::error!("Help me out");
}

#[test]
fn logs_with_arg() {
    foundation_core::info!("Help me out: {}", 1);
    foundation_core::debug!("Help me out: {}", 1);
    foundation_core::warn!("Help me out: {}", 1);
    foundation_core::error!("Help me out: {}", 1);
}
