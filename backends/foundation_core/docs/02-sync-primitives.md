# Sync primitives for wasm and native

## What
Thread-safe primitives that work identically on native (OS threads) and wasm
(single-threaded event loop).

## Key types
- **`MppChannel<T>`** — Multi-producer, multi-consumer channel. Uses `AtomicU64`
  for seekable cursors.
- **`WaitGroup`** — Wait for N tasks to complete. Similar to Go's `sync.WaitGroup`.
- **`Idleman`** — Idle detection: signal when all tasks have quiesced.
- **`Event`** — Async event handle for one-shot signaling.
- **Spin locks/mutexes** — `SpinMutex`, `SpinRwLock` — optimized for short
  critical sections, wasm-compatible.

## Wasm considerations
On wasm32-unknown-unknown there are no threads. Primitives fall back to
no-op or event-loop-friendly implementations. `SendWrapper` allows `!Send`
values to satisfy `Send` bounds safely on single-threaded targets.
