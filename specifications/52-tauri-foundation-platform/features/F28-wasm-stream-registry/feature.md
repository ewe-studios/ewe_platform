---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F28-wasm-stream-registry"
this_file: "specifications/52-tauri-foundation-platform/features/F28-wasm-stream-registry/feature.md"

status: completed
priority: high
created: 2026-07-21

depends_on:
  - "F25-ipc-registry"
  - "F26-streaming-channels"
  - "F27-wasm-runtime-triggers"

tasks:
  completed: 0
  uncompleted: 9
  total: 9
  completion_percentage: 0%
---
# F28 — WASM ConcurrentQueue Stream Registry + JS Stream Objects

## Problem

F26 implements streaming IPC for the **platform** (Tauri Channels). The WASM
side has no equivalent. WASM code running in the browser, Deno, or Tauri
webview cannot:

1. **Stream responses back to the host** — no `ConcurrentQueue`-backed stream
   registry that the WASM side writes to and the JS host drains.
2. **Receive streams from the host** — no JS object that represents an incoming
   data stream with `onData` / `onEnd` / `onError` callbacks.

The platform already has Tauri Channels for this. But non-Tauri hosts
(browser, Deno, testbed) need a portable, lock-free mechanism.

## Solution

Two halves, both in `foundation_wasm`:

### Half 1: `StreamRegistry` — WASM → host streaming

A `concurrent_queue::ConcurrentQueue`-based registry. WASM code creates a
stream, gets a `StreamId`, pushes chunks via `send()`, and calls `close()`
when done. The JS host drains the queue.

```rust
// foundation_wasm/src/stream.rs (NEW)

pub struct StreamId(pub u64);

pub struct StreamChunk {
    pub data: Vec<u8>,
    pub sequence: u64,
}

pub struct StreamRegistry {
    next_id: u64,
    streams: Mutex<BTreeMap<StreamId, ConcurrentQueue<StreamChunk>>>,
    // On wasm32: plain BTreeMap (single-threaded)
}

impl StreamRegistry {
    /// Create a new stream, returns (StreamId, queue for host to drain).
    pub fn create(&self) -> (StreamId, ConcurrentQueue<StreamChunk>);

    /// Push a chunk. Returns false if stream doesn't exist.
    pub fn send(&self, id: StreamId, chunk: StreamChunk) -> bool;

    /// Close and deregister. Returns false if already closed.
    pub fn close(&self, id: StreamId) -> bool;
}
```

**WASM API** (exported to JS via wasm-bindgen or `#[no_mangle]`):
- `stream_create() -> u64` — create a stream, returns StreamId
- `stream_send(id: u64, data_ptr: *const u8, data_len: usize, seq: u64) -> bool`
- `stream_close(id: u64) -> bool`

**JS host drains the queue**: After `stream_create()`, the JS side gets the
`ConcurrentQueue` (via a registry export) and polls/iterates it, delivering
chunks to the registered callback.

### Half 2: JS stream objects — host → WASM streaming

The JS host provides two objects for incoming stream handling:

```js
// Incoming stream — WASM receives data FROM the host.
// Usage:
//   var stream = new WasmStreamReceiver({
//       onData: function(chunk) { /* chunk = { data: Uint8Array, sequence: number } */ },
//       onEnd: function() { /* stream complete */ },
//       onError: function(err) { /* error */ }
//   });
//   wasmApp.handleStream("upload", stream);
function WasmStreamReceiver(opts) {
    this._onData = opts.onData || noop;
    this._onEnd = opts.onEnd || noop;
    this._onError = opts.onError || noop;
    this._id = nextStreamId++;
    streamReceivers[this._id] = this;
}

// Called by WASM (through host_apply) when data arrives:
WasmStreamReceiver.push = function(id, data, sequence, isLast) {
    var r = streamReceivers[id];
    if (!r) return;
    try {
        if (isLast) {
            r._onEnd();
        } else {
            r._onData({ data: data, sequence: sequence });
        }
    } catch (e) {
        r._onError(e);
    }
};

// Outgoing stream — WASM sends data TO the host.
// Usage:
//   var stream = new WasmStreamSender(function onChunk(chunk) {
//       // deliver chunk to host's IPC/capability transport
//   }, function onEnd() {
//       // stream complete
//   });
//   wasmApp.startStream("file-read", stream);
function WasmStreamSender(onChunk, onEnd) {
    this._onChunk = onChunk || noop;
    this._onEnd = onEnd || noop;
    this._id = nextStreamId++;
    streamSenders[this._id] = this;
}

// Called by WASM when it has a chunk to send:
WasmStreamSender.send = function(id, data, sequence) {
    var s = streamSenders[id];
    if (s) s._onChunk({ data: data, sequence: sequence });
};

// Called by WASM when the stream is done:
WasmStreamSender.end = function(id) {
    var s = streamSenders[id];
    if (s) { s._onEnd(); delete streamSenders[id]; }
};
```

The WASM side creates these objects by calling JS imports:

```rust
// foundation_wasm host imports (via wasm-bindgen or raw FFI):
#[wasm_bindgen]
extern "C" {
    fn wasm_stream_receiver_push(id: u64, data_ptr: *const u8, data_len: usize, seq: u64, is_last: bool);
    fn wasm_stream_sender_send(id: u64, data_ptr: *const u8, data_len: usize, seq: u64);
    fn wasm_stream_sender_end(id: u64);
}
```

### How `foundation_wasm_ui` builds on this

`foundation_wasm_ui` wraps the raw stream primitives in its reactive system:

```js
// foundation_wasm_ui can provide reactive stream bindings:
function reactiveStream(signal, opts) {
    var sender = new WasmStreamSender(
        function(chunk) { signal.value = chunk; },
        function() { signal.done = true; }
    );
    return sender;
}
```

### How `foundation_platform` already handles this

The platform uses Tauri Channels for streaming (F26). The `StreamRegistry`
and JS stream objects are the WASM-level equivalents — they work on any host
(browser, Deno, Tauri webview) without platform dependencies. The platform
can also wrap them: a `PlatformStreamRegistry` handler (F26) that delegates
to the WASM `StreamRegistry` when running inside a webview.

## Requirements

### 1. `StreamRegistry` — `foundation_wasm`
- File: `backends/foundation_wasm/src/stream.rs` (NEW)
- `StreamId(u64)`, `StreamChunk { data, sequence }`
- `create()` → `(StreamId, ConcurrentQueue<StreamChunk>)`
- `send(id, chunk)` → `bool`
- `close(id)` → `bool`
- `Mutex<BTreeMap<...>>` on native, plain `BTreeMap` on wasm32
- Uses `concurrent_queue::ConcurrentQueue` — already a workspace dep
- `concurrent-queue` added to `foundation_wasm` Cargo.toml
- Thread-safe `register(&self)` on native (interior mutability)

### 2. WASM exports — `foundation_wasm`
- `stream_create() -> u64`
- `stream_send(id, data_ptr, data_len, seq) -> bool`
- `stream_close(id) -> bool`
- Exported as `#[no_mangle] pub extern "C" fn` for wasm32

### 3. `WasmStreamReceiver` JS object
- Constructor takes `{ onData, onEnd, onError }` callbacks
- `WasmStreamReceiver.push(id, data, sequence, isLast)` — called by WASM
- Lives in `foundation-wasm.js` runtime
- IDs are managed by the JS side (monotonic counter)

### 4. `WasmStreamSender` JS object
- Constructor takes `onChunk(data, sequence)` and `onEnd()` callbacks
- `WasmStreamSender.send(id, data, sequence)` — WASM pushes a chunk
- `WasmStreamSender.end(id)` — WASM signals end, sender deregistered
- Lives in `foundation-wasm.js` runtime

### 5. WASM→JS FFI imports
- `wasm_stream_receiver_push(id, data_ptr, data_len, seq, is_last)`
- `wasm_stream_sender_send(id, data_ptr, data_len, seq)`
- `wasm_stream_sender_end(id)`
- Behind `web` feature gate (needs JS host)

### 6. `foundation_wasm_ui` integration
- Reactive wrappers around `WasmStreamSender`/`WasmStreamReceiver`
- `reactiveStreamSender(signal)` — binds WASM stream to reactive signal

### 7. Relationship to `foundation_platform` (F26)
- `foundation_platform` already has Tauri Channels (F26) for streaming —
  `__ewe_ipc_stream` command, `PlatformStreamRegistry`, `StreamingIpc` trait.
  No changes needed there.
- This feature provides the *wasm-side equivalent* for non-Tauri hosts
  (browser, Deno, testbed): `StreamRegistry` for WASM→host, and
  `WasmStreamReceiver`/`WasmStreamSender` JS objects for host↔WASM.
- Tauri hosts use their native Channel infrastructure to talk to WASM;
  non-Tauri hosts use these JS stream objects as the bridge.
- A future integration layer could let `PlatformStreamRegistry` handlers
  delegate to the WASM `StreamRegistry` when running inside a webview,
  unifying the two paths — but this is not in scope for F28.

### 8. No std::sync::mpsc
- `concurrent_queue::ConcurrentQueue` only — lock-free, wasm32-safe

### 9. Tests
- Unit tests for `StreamRegistry` create/send/close
- JS tests for `WasmStreamReceiver`/`WasmStreamSender` via CDP/BiDi

## Verification

```bash
# Rust side
cargo check -p foundation_wasm
cargo test -p foundation_wasm -- stream

# JS runtime — verify objects exist
# Browser console:
# > typeof WasmStreamReceiver  // "function"
# > typeof WasmStreamSender    // "function"
# > var r = new WasmStreamReceiver({ onData: console.log, onEnd: () => console.log('done') });
# > WasmStreamReceiver.push(r._id, new Uint8Array([1,2,3]), 0, false);
# // logs: { data: Uint8Array[1,2,3], sequence: 0 }
# > WasmStreamReceiver.push(r._id, new Uint8Array([]), 1, true);
# // logs: done

# Integration: WASM creates a stream, sends chunks, JS receives them
# via the WasmStreamReceiver bridge
```

## Files

| File | Action |
|------|--------|
| `backends/foundation_wasm/src/stream.rs` | **NEW** — StreamRegistry, StreamId, StreamChunk |
| `backends/foundation_wasm/src/lib.rs` | Add `pub mod stream;` |
| `backends/foundation_wasm/Cargo.toml` | Add `concurrent-queue` dep |
| `backends/foundation_wasm/runtime/foundation-wasm.js` | Add `WasmStreamReceiver`, `WasmStreamSender` |
| `backends/foundation_wasm/src/host_runtime.rs` | Add wasm_stream_* FFI imports |
| `backends/foundation_wasm/tests/stream_tests.rs` | **NEW** — StreamRegistry tests |
| `backends/foundation_wasm_ui/runtimes/foundation-wasm-ui.js` | Reactive stream bindings |
