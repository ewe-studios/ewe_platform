//! Regression test for issue #426: the WASI preview 1 context must take its
//! clock and randomness from the injected `Ctx`, not from the host globals.
//!
//! WHY: before the fix `build` called `WasiCtxBuilder::new().build_p1()`, so a
//! guest read the real wall clock and the process RNG. A test could not control
//! either value, and `foundation_wasmtime` broke the rule that every side
//! effect comes from the injected bundle (spec 70 rule 11).
//!
//! WHAT: the module calls `clock_time_get` and `random_get`. One request byte
//! selects the value: `0` reads the realtime clock, `1` the monotonic clock,
//! `2` eight random bytes. Each call writes eight bytes at offset `0` and the
//! guest returns them as the response body.
//!
//! HOW: `SimulatedClock` fixes both clocks, and a fixed `ChaCha8Rng` seed
//! fixes the random stream. Each assertion then names the injected value
//! exactly. The old code fails every assertion: it returns the real time and a
//! different random stream.

use std::sync::Arc;
use std::time::Duration;

use foundation_compact::rng::{ChaCha8Rng, Rng, SeedableRng};
use foundation_core::valtron::{Ctx, SimulatedClock};
use foundation_wasmtime::WasmtimeBuilder;

/// A module that reports the injected clock or randomness. The request byte
/// selects the value:
///
/// - `0`: `clock_time_get(REALTIME)` at offset `0`.
/// - `1`: `clock_time_get(MONOTONIC)` at offset `0`.
/// - `2`: `random_get(offset 0, len 8)`.
///
/// The response is the eight bytes at offset `0`.
const ENV_MODULE: &str = r#"
(module
  (import "wasi_snapshot_preview1" "clock_time_get"
    (func $clock_time_get (param i32 i64 i32) (result i32)))
  (import "wasi_snapshot_preview1" "random_get"
    (func $random_get (param i32 i32) (result i32)))

  (memory (export "memory") 1)

  (func (export "ewe_alloc") (param $size i32) (result i32)
    i32.const 1024)

  (func (export "handle_request") (param $ptr i32) (param $len i32) (result i64)
    (local $mode i32)
    (local.set $mode (i32.load8_u (local.get $ptr)))
    (if
      (i32.eqz (local.get $mode))
      (then
        ;; REALTIME = 0, one nanosecond of precision, result at offset 0.
        (drop (call $clock_time_get (i32.const 0) (i64.const 1) (i32.const 0))))
      (else
        (if
          (i32.eq (local.get $mode) (i32.const 1))
          (then
            ;; MONOTONIC = 1, one nanosecond of precision, result at offset 0.
            (drop (call $clock_time_get (i32.const 1) (i64.const 1) (i32.const 0))))
          (else
            (drop (call $random_get (i32.const 0) (i32.const 8)))))))
    (i64.const 8)))
"#;

/// Build an instance over [`ENV_MODULE`] with `ctx`.
fn instance_with(ctx: &Ctx) -> foundation_wasmtime::WasmtimeInstance {
    let bytes = wat::parse_str(ENV_MODULE).expect("the WAT source parses");
    WasmtimeBuilder::new(bytes)
        .with_name("env")
        .build(ctx)
        .expect("the guest links and instantiates")
}

#[test]
fn realtime_clock_is_the_injected_clock() {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    clock.advance_wall(Duration::from_secs(1_700_000_000));
    let ctx = Ctx::new(clock);
    let mut instance = instance_with(&ctx);

    let body = instance
        .call_handle_request(&[0])
        .expect("the guest reads the realtime clock");

    assert_eq!(
        body,
        1_700_000_000u64
            .saturating_mul(1_000_000_000)
            .to_le_bytes()
            .to_vec(),
        "the guest must read the session's wall clock, not the host clock"
    );
}

#[test]
fn monotonic_clock_is_the_injected_clock() {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    clock.advance(Duration::from_secs(42));
    let ctx = Ctx::new(clock);
    let mut instance = instance_with(&ctx);

    let body = instance
        .call_handle_request(&[1])
        .expect("the guest reads the monotonic clock");

    assert_eq!(
        body,
        42_000_000_000u64.to_le_bytes().to_vec(),
        "the guest must read the session's monotonic clock, not the host clock"
    );
}

#[test]
fn guest_randomness_is_the_injected_rng() {
    let ctx = Ctx::new(Arc::new(SimulatedClock::new(Duration::ZERO)));
    let mut expected_rng = ChaCha8Rng::from_seed([0x5a; 32]);
    *ctx.rng().lock().expect("the ctx RNG lock is not poisoned") = expected_rng.clone();
    // `wasmtime-wasi` fills a random request with `sample_iter(Standard)`. For
    // a byte, the standard distribution takes the low byte of one `next_u32`
    // draw, so the expected stream is eight low bytes, not eight stream bytes.
    let mut expected = [0u8; 8];
    for byte in &mut expected {
        // The low byte of each draw, as the standard distribution takes it.
        *byte = expected_rng.next_u32().to_le_bytes()[0];
    }

    let mut instance = instance_with(&ctx);
    let body = instance
        .call_handle_request(&[2])
        .expect("the guest reads random bytes");

    assert_eq!(
        body,
        expected.to_vec(),
        "the guest must draw from the session's RNG, not the process RNG"
    );
}
