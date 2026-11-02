// Copyright (c) 2019 Cloudflare, Inc. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! A wall-clock `Instant` for wasm32, built on `js_sys::Date::now()`.
//!
//! WHY: wasm32-unknown-unknown has no `std::time::Instant` (it panics at
//! runtime), and boringtun's other sleepyinstant backends cover only unix and
//! windows. This backend lets the pure-Rust WireGuard noise state machine
//! compile and run in browsers, Deno and Cloudflare Workers.
//!
//! NOTE: `Date::now()` is wall-clock time, not a monotonic clock, so a clock
//! adjustment can move an instant backwards. boringtun uses `Instant` only for
//! handshake timers, RTT and rate limiting, and `duration_since` clamps a
//! negative difference to `Duration::ZERO`, so the state machine stays sound.

use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Instant {
    ms: f64,
}

impl Instant {
    pub(crate) fn now() -> Self {
        Self {
            ms: js_sys::Date::now(),
        }
    }

    pub(crate) fn duration_since(&self, earlier: Instant) -> Duration {
        let diff_ms = self.ms - earlier.ms;
        if diff_ms <= 0.0 {
            return Duration::ZERO;
        }
        Duration::from_secs_f64(diff_ms / 1000.0)
    }
}
