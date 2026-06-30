# Per-Host Scheduling Model

foundation_wasm's scheduling (timeouts, intervals, animation frames) works differently
depending on the host backend. The two models: **host-driven** (web/JS) and
**guest-driven** (WASI).

## Web Host (JS event loop) — `web` feature

On `wasm32-unknown-unknown` and `wasm32-unknown-emscripten`, the JS host owns the event
loop. foundation_wasm delegates timing to the host:

```
Guest (wasm)                          Host (JS)
────────────                          ─────────
register_schedule(50ms, callback)
  → stores callback in SCHEDULED_CALLBACKS
  → calls abi::web::schedule_timeout(50, id)  ──→  setTimeout(50, () => host_apply(id))
                                                        │
  ← host_apply(id)  ←──────────────────────────────────┘
  → looks up id in SCHEDULED_CALLBACKS
  → fires callback
```

Key properties:
- **Host-driven**: the JS event loop (browser `setTimeout`/`setInterval`/
  `requestAnimationFrame`) decides when to fire.
- No polling — the guest registers and waits.
- `host_apply` is the single re-entry point from JS into wasm.
- Animation frames use `requestAnimationFrame` with a monotonic timestamp.
- Works in browser and Deno (both have the JS event loop).

Source: `src/host_runtime.rs` — `abi::web::schedule_timeout`, `schedule_interval`,
`request_animation_frame` (wasm_import_module = "abi" FFI imports).

## WASI Host (timer-based polling) — `wasi` / `wasip2` features

On `wasm32-wasip1` and `wasm32-wasip2`, there is no JS event loop. The `wasi_host`
module provides a guest-driven polling model backed by `std::time`:

```
Guest (wasm)                          WASI Host
────────────                          ─────────
register_schedule(50ms, callback)
  → stores callback in SCHEDULED_CALLBACKS (same registry)
  → stores TimeoutEntry { deadline: Instant::now() + 50ms } in TIMERS

// option A: guest drives the loop
loop {
    tick()                            (no host involvement)
      → checks Instant::now() >= deadline
      → fires expired callbacks
      → returns STOP when idle
}

// option B: guest blocks efficiently
poll_blocking()
  → time_until_next_event() → Some(48ms)
  → std::thread::sleep(48ms)         → WASI clock_time_get / wasi:clocks
  → tick()
  → returns STOP or REQUEUE

// option C: host drives via exports
                                      wasi_tick() ──→ tick()
                                      wasi_poll_blocking() ──→ poll_blocking()
                                      wasi_time_until_next_event_ms() ──→ time_until_next_event()
                                      wasi_has_pending_work() ──→ has_pending_work()
```

Key properties:
- **Guest-driven**: the wasm module decides when to poll. No external callback mechanism.
- `tick()` is non-blocking — checks deadlines, fires what's due, returns immediately.
- `poll_blocking()` sleeps until the next event — the simplest event loop is
  `loop { poll_blocking(); }`.
- `time_until_next_event()` enables integration with external event loops (e.g. an
  embedder can sleep for that duration, then call `wasi_tick()`).
- Timing uses `std::time::Instant` which maps to `clock_time_get` (wasip1) or
  `wasi:clocks/monotonic-clock` (wasip2) — both provide monotonic nanosecond resolution.
- Animation frames use a lazy-initialized `START_TIME` for monotonic seconds.

Source: `src/wasi_host.rs`, `src/host_runtime.rs` (exposed_runtime wasi_* exports).

## Shared Infrastructure

Both hosts share the same callback registries:

| Registry | Purpose |
|----------|---------|
| `SCHEDULED_CALLBACKS` | One-shot timeouts (setTimeout / register_schedule) |
| `RECURRING_INTERVAL_CALLBACKS` | Repeating intervals (setInterval / register_interval) |
| `ANIMATION_FRAME_CALLBACKS` | Frame callbacks (requestAnimationFrame / monotonic tick) |
| `INTERNAL_CALLBACKS` | Host-apply operation callbacks |

The `internal_api` module provides the registration/lookup/fire functions that both
backends use. The difference is only in **who decides when to fire**: the JS host
(via imported FFI calls) or the wasm guest (via `tick()`/`poll_blocking()`).

## Choosing a Host

| Scenario | Feature | Model |
|----------|---------|-------|
| Browser SPA, CF Workers | `web` | Host-driven (JS event loop) |
| Browser + emscripten (llama/WebGPU) | `web` | Host-driven (JS event loop) |
| Server-side wasm (wasmtime, Deno WASI) | `wasi` | Guest-driven (poll/tick) |
| Component-model deployment | `wasip2` | Guest-driven (poll/tick) |
| Native (testing, dev) | `wasi` | Guest-driven (std::time::Instant) |

The `wasi` feature compiles and runs on native too — `std::time::Instant` works
everywhere — which is how the wasi_host tests run under `cargo test`.
