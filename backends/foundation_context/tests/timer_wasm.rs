//! The wasm fallback for [`Ctx::try_timer`].
//!
//! A wasm context with no `Env` timer has no default timer yet. The call must
//! return a typed error, never hang. This test file compiles only for a wasm
//! target.

#![cfg(target_family = "wasm")]

use std::sync::Arc;

use foundation_context::{Ctx, MonotonicClock, TimerError};

#[test]
fn wasm_without_env_timer_is_a_typed_error() {
    let ctx = Ctx::fixed(Arc::new(MonotonicClock::new()));
    assert!(matches!(ctx.try_timer(), Err(TimerError::Missing)));
}
