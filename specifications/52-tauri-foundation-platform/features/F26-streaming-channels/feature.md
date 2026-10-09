---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F26-streaming-channels"
this_file: "specifications/52-tauri-foundation-platform/features/F26-streaming-channels/feature.md"

status: completed
priority: medium
created: 2026-07-20
updated: 2026-07-21

depends_on:
  - "F25-ipc-registry"

tasks:
  completed: 5
  uncompleted: 0
  total: 5
  completion_percentage: 100%
---
# F26 — Streaming Channels: Tauri Channels for ordered data delivery

## Problem

Tauri v2 provides **Channels** — an ordered, high-throughput streaming
primitive. Unlike events, channels are ordered, typed, and bidrectional.
F25's `Ipc` trait is request/response (`fn invoke() -> Result<IpcResponse, IpcError>`).
Streaming needs a different contract: multiple chunks, bidirectional flow,
and backpressure.

Constraint: no tokio. Platform async is `valtron`.

## Solution: Tauri Channel internals (grounded in source)

### The full Rust-side flow

Every IPC call starts with `InvokeRequest` arriving at `Webview::on_message()`
(`webview/mod.rs:1742`):

```rust
// webview/mod.rs:131-146
pub struct InvokeRequest {
    pub cmd: String,            // e.g. "load_image"
    pub callback: CallbackFn,   // u32 — success callback ID
    pub error: CallbackFn,      // u32 — error callback ID
    pub url: Url,               // origin URL of the frame
    pub body: InvokeBody,       // Json(Value) or Raw(Vec<u8>)
    pub headers: HeaderMap,
    pub invoke_key: String,     // __TAURI_INVOKE_KEY__ verification
}
```

`Webview::on_message()` constructs an `InvokeResolver` holding the webview
and the responder closure, then hands everything to `InvokeMessage`:

```rust
// ipc/mod.rs:286-292
pub struct InvokeResolver<R: Runtime> {
    webview: Webview<R>,
    responder: Arc<Mutex<Option<Box<OwnedInvokeResponder<R>>>>>,
    cmd: String,
    pub(crate) callback: CallbackFn,   // the JS callback IDs
    pub(crate) error: CallbackFn,
}
```

The command dispatcher calls our `#[tauri::command]` function. If the
function has a `Channel<T>` parameter, `CommandArg` deserializes it:

```rust
// channel.rs:300-316
impl<'de, R: Runtime, TSend> CommandArg<'de, R> for Channel<TSend> {
    fn from_command(command: CommandItem<'de, R>) -> Result<Self, InvokeError> {
        // 1. Deserialize the arg as a String → "__CHANNEL__:123"
        let value: String = Deserialize::deserialize(command)?;
        // 2. Parse the numeric ID
        JavaScriptChannelId::from_str(&value)
            .map(|id| id.channel_on(webview))  // 3. Construct the Rust Channel
    }
}
```

`JavaScriptChannelId::channel_on()` (`channel.rs:134-194`) is the heart of
channel construction:

```rust
// channel.rs:134-194
impl JavaScriptChannelId {
    pub fn channel_on<R: Runtime, TSend>(&self, webview: Webview<R>) -> Channel<TSend> {
        let callback_id = self.0 .0;  // the u32 from "__CHANNEL__:123"

        Channel::new_with_id(
            callback_id,
            // on_message: called for every Channel::send()
            Box::new(move |body: InvokeResponseBody| {
                match body {
                    // Small JSON (≤8192 bytes) → eval directly
                    InvokeResponseBody::Json(json)
                        if json.len() < 8192 =>
                    {
                        webview.eval(format_raw_js(callback_id, json))?;
                    }
                    // Small raw (≤1024 bytes) → eval as Uint8Array
                    InvokeResponseBody::Raw(bytes)
                        if bytes.len() < 1024 =>
                    {
                        let arr = serde_json::to_string(&bytes)?;
                        webview.eval(format_raw_js(
                            callback_id,
                            format!("new Uint8Array({arr}).buffer"),
                        ))?;
                    }
                    // Large payloads → store in queue, JS fetches via IPC
                    _ => {
                        let data_id = CHANNEL_DATA_COUNTER.fetch_add(1, Relaxed);
                        webview.state::<ChannelDataIpcQueue>()
                            .0.lock().unwrap()
                            .insert(data_id, body);
                        // Kick off a fetch on the JS side
                        webview.eval(format!(
                            "window.__TAURI_INTERNALS__.invoke(
                                'plugin:__TAURI_CHANNEL__|fetch', null,
                                {{ headers: {{ 'Tauri-Channel-Id': '{data_id}' }} }}
                            ).then(r => window.__TAURI_INTERNALS__.runCallback(
                                {callback_id}, {{ message: r, index: {current_index} }}
                            )).catch(console.error)",
                        ))?;
                    }
                }
                Ok(())
            }),
            // on_drop: fires when the Rust Channel is dropped
            Some(Box::new(move || {
                webview.eval(format_raw_js(
                    callback_id,
                    format!("{{ end: true, index: {current_index} }}"),
                )).ok();
            })),
        )
    }
}
```

**Key facts from this:**
1. `Channel::send()` is **synchronous** — calls `on_message(body)` directly.
   The `async` on the command fn is a Tauri framework requirement, not a
   channel requirement.
2. Small payloads go direct via `webview.eval()`, large ones via fetch API
   + `ChannelDataIpcQueue`. This is transparent to us.
3. **End-of-stream is automatic**: when the Rust `Channel` drops, `on_drop`
   fires `{ end: true, index: N }` to JS. We don't need an `is_last` flag
   — Tauri already signals stream termination.
4. The `on_message` closure captures `callback_id` — the same ID the JS
   `Channel` class used when calling `transformCallback`.

`Channel::send()` itself (`channel.rs:292-298`):

```rust
pub fn send(&self, data: TSend) -> crate::Result<()>
where
    TSend: IpcResponse,
{
    (self.inner.on_message)(data.body()?)
}
```

The `IpcResponse` trait (`ipc/mod.rs:176-187`):

```rust
pub trait IpcResponse {
    fn body(self) -> crate::Result<InvokeResponseBody>;
}

// Blanket impl: anything Serialize → IpcResponse
impl<T: Serialize> IpcResponse for T {
    fn body(self) -> crate::Result<InvokeResponseBody> {
        serde_json::to_string(&self).map(Into::into).map_err(Into::into)
    }
}

// InvokeResponseBody carries either Json(String) or Raw(Vec<u8>)
impl IpcResponse for InvokeResponseBody {
    fn body(self) -> crate::Result<InvokeResponseBody> { Ok(self) }
}
```

This means `Channel::send(our_chunk)` will:
- If `our_chunk: Serialize` → serialize to JSON string, wrap in `InvokeResponseBody::Json`
- If `our_chunk: InvokeResponseBody` → pass through directly (raw bytes)

### Mobile: the global CHANNELS map

On mobile, `Channel::new()` also registers the channel globally (`channel.rs:234-238`):

```rust
#[cfg(mobile)]
crate::plugin::mobile::register_channel(Channel {
    inner: channel.inner.clone(),
    phantom: Default::default(),
});
```

`plugin/mobile.rs:35,58-64`:

```rust
static CHANNELS: OnceLock<Mutex<HashMap<u32, Channel<serde_json::Value>>>> = OnceLock::new();

pub(crate) fn register_channel(channel: Channel<serde_json::Value>) {
    CHANNELS.get_or_init(Default::default)
        .lock().unwrap()
        .insert(channel.id(), channel);
}
```

On Android, Kotlin code can look up a channel by ID and send data through
JNI into it. The channel data flows through the platform bridge, not through
`webview.eval()`.

### The JS Channel class

```js
// From bundle.global.js (relevant parts, pure JS)

function uid() {
    return window.crypto.getRandomValues(new Uint32Array(1))[0];
}

// Internals that Channel depends on:
//   window.__TAURI_INTERNALS__.transformCallback(cb, once) → numeric callback ID
//   window.__TAURI_INTERNALS__.unregisterCallback(id)
//   window.__TAURI_INTERNALS__.runCallback(id, data)

function Channel(onmessage) {
    this.id = null;
    this._nextIndex = 0;
    this._pending = {};
    this._pendingCleanup = null;

    var self = this;

    this.id = window.__TAURI_INTERNALS__.transformCallback(function(raw) {
        // End-of-stream from Rust (Channel dropped)
        if ('end' in raw) {
            if (raw.index === self._nextIndex) {
                window.__TAURI_INTERNALS__.unregisterCallback(self.id);
            } else {
                self._pendingCleanup = raw.index;
            }
            return;
        }

        var msg = raw.message;

        if (raw.index === self._nextIndex) {
            if (self.onmessage) self.onmessage(msg);
            self._nextIndex++;
            // Drain pending out-of-order messages
            while (self._nextIndex in self._pending) {
                if (self.onmessage) self.onmessage(self._pending[self._nextIndex]);
                delete self._pending[self._nextIndex];
                self._nextIndex++;
            }
            if (self._nextIndex === self._pendingCleanup) {
                window.__TAURI_INTERNALS__.unregisterCallback(self.id);
            }
        } else {
            // Buffer out-of-order message
            self._pending[raw.index] = msg;
        }
    });
}

// Wire format serialization
Channel.prototype.__TAURI_TO_IPC_KEY__ = function() {
    return '__CHANNEL__:' + this.id;
};
Channel.prototype.toJSON = function() {
    return this.__TAURI_TO_IPC_KEY__();
};
```

**How a JS Channel fires:**
1. `new Channel()` → `transformCallback` registers a callback, stores the numeric ID
2. When passed to `invoke()`, serializes as `"__CHANNEL__:123"`
3. Rust deserializes this → `JavaScriptChannelId(123)` → `channel_on(webview)` → `Channel`
4. Each `channel.send(chunk)` on Rust side calls the `on_message` closure with `{ message: chunk, index: N }`
5. JS callback receives `{ message, index }`, calls `self.onmessage(msg)`
6. When Rust Channel drops, `on_drop` fires `{ end: true, index: N }`
7. JS callback unregisters itself

### Summary of what Tauri gives us

| Mechanism | Delivery | Overhead | End-of-stream |
|-----------|----------|----------|---------------|
| `eval()` direct | Inline JS eval | Zero | Drop → `{ end: true }` |
| `ChannelDataIpcQueue` + fetch | IPC round-trip | One fetch per large chunk | Drop → `{ end: true }` |
| Mobile `CHANNELS` map | JNI → Kotlin → WebView | Platform bridge | Drop → platform signal |

All three paths are transparent. `Channel::send()` → `on_message` → delivery.
End-of-stream is always signaled by Channel drop, never missed.

## Design

`StreamingIpc` extends `Ipc` (F25) with two methods. It is NOT an `IpcKind`
variant — the `Ipc` trait is request/response. Streaming is a different contract.

```rust
/// A streaming IPC — extends the base `Ipc` trait with bidirectional chunk
/// delivery. Not an `IpcKind` variant; this is a different contract.
///
/// `Ipc::kind()` returns `IpcKind::Query` — streaming is the extended trait,
/// not a kind. The initial invoke establishes the stream; the extended trait
/// methods handle the chunk flow.
pub trait StreamingIpc: Ipc {
    /// Server → client: emit a stream of chunks to the frontend.
    ///
    /// Called once per invocation. The platform drains the returned
    /// `IpcStreamReceiver` into the Tauri Channel, delivering each
    /// chunk to JS via `onChunk`. End-of-stream is signaled when
    /// the Rust Channel drops (Tauri sends `{ end: true }` to JS).
    ///
    /// Cancellation: if the JS side calls `stream.cancel()`, the
    /// Tauri Channel closes, subsequent `Channel::send()` calls
    /// return `Err`, and the platform stops draining.
    fn stream(
        &self,
        session: &PlatformSession,
        request: IpcRequest,
    ) -> Result<IpcStream, IpcError>;

    /// Client → server: accept a stream of chunks FROM the frontend.
    ///
    /// For uploads, sensor feeds, voice input, etc. The platform
    /// collects chunks from the JS side into an `IpcStreamReceiver`,
    /// then calls this method to process them.
    ///
    /// Returns a single `IpcResponse` when the input stream is
    /// complete (the final chunk arrives, or the JS side closes).
    fn accept_stream(
        &self,
        session: &PlatformSession,
        request: IpcRequest,
        input: IpcStreamReceiver,
    ) -> Result<IpcResponse, IpcError>;
}

pub struct IpcStream {
    pub receiver: IpcStreamReceiver,
    pub total_hint: Option<u64>,
}

pub enum IpcStreamReceiver {
    /// Lock-free concurrent queue. Already a workspace dep (foundation_core,
    /// foundation_ai use `concurrent_queue::ConcurrentQueue`). Works on wasm32
    /// and native. No `std::sync::mpsc` needed.
    Sync(concurrent_queue::ConcurrentQueue<Result<IpcStreamChunk, IpcError>>),
    #[cfg(feature = "async")]
    Async(Box<dyn Stream<Item = Result<IpcStreamChunk, IpcError>> + Send + Unpin>),
}

pub struct IpcStreamChunk {
    pub data: Vec<u8>,
    pub sequence: u64,
    pub progress: Option<u64>,
}

impl Serialize for IpcStreamChunk {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer
    {
        // Serialize as JSON for Tauri's IpcResponse blanket impl.
        // Binary data → base64 in the JSON so JS can decode.
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("IpcStreamChunk", 3)?;
        s.serialize_field("data", &base64_encode(&self.data))?;
        s.serialize_field("sequence", &self.sequence)?;
        s.serialize_field("progress", &self.progress)?;
        s.end()
    }
}
```

Note: `IpcStreamChunk` does NOT have `is_last`. Tauri's Channel drop provides
end-of-stream via `{ end: true }`. We don't duplicate it.

### Bidirectional data flow

**Server → client** (download, log tail, sensor feed):

```
JS:  var stream = invokeIpcStream('filesystem', 'read', { path: '...' });
     // Under the hood:
     //   var ch = new Channel();
     //   ch.onmessage = function(chunk) { stream._onChunk(chunk); };
     //   window.__TAURI_INTERNALS__.invoke('__ewe_ipc_stream', {
     //       ipc: 'filesystem',
     //       action: 'read',
     //       payload: [1,2,3],  // serialized bytes
     //       content_type: 'application/json',
     //       stream: ch         // serializes as "__CHANNEL__:123"
     //   });

Rust: #[tauri::command] fn __ewe_ipc_stream(..., stream: Channel<IpcStreamChunk>)
      → PlatformStreamRegistry::get("filesystem")
        → StreamingIpc::stream(session, request) → IpcStream { receiver }
      → loop: receiver.recv() → chunk → stream.send(chunk)
      → return Ok(())  // Channel drops here → Tauri sends { end: true } to JS

JS:  chunk arrives → ch.onmessage fires → stream._onChunk(chunk)
     when { end: true } arrives → stream._onDone()
```

**Client → server** (upload, voice input, data sync):

```
JS:  var upload = createUploadStream('filesystem', 'write', { path: '/tmp/out.bin' });
     // Creates a JS Channel, calls:
     //   window.__TAURI_INTERNALS__.invoke('__ewe_ipc_stream_accept', {
     //       ipc: 'filesystem',
     //       action: 'write',
     //       payload: [...],       // request params
     //       content_type: 'application/json',
     //       input: uploadChannel  // "__CHANNEL__:456"
     //   });
     upload.send(new Uint8Array([1,2,3]));
     upload.send(new Uint8Array([4,5,6]));
     upload.close();

Rust: #[tauri::command] fn __ewe_ipc_stream_accept(..., input: Channel<IpcStreamChunk>)
      → collects chunks from the input channel into an IpcStreamReceiver
      → StreamingIpc::accept_stream(session, request, receiver)
      → returns IpcResponse when complete
      → IpcResponse delivered via the command's normal return path

JS:  await upload.result → the IpcResponse from accept_stream()
```

### Tauri commands

Two dedicated commands, separate from `__ewe_ipc` (F25) and `__ewe_capabilities` (F23):

```rust
/// Server → client: streaming output.
///
/// The `stream` argument is a Channel<IpcStreamChunk> deserialized from
/// the JS Channel's "__CHANNEL__:ID" wire format.
#[tauri::command]
async fn __ewe_ipc_stream(
    app_handle: tauri::AppHandle,
    ipc: String,
    action: String,
    payload: Vec<u8>,
    content_type: String,
    stream: tauri::ipc::Channel<IpcStreamChunk>,
) -> Result<(), String> {
    let session = app_handle.state::<PlatformSession>();
    let request = IpcRequest {
        ipc, action, payload,
        content_type: content_type.parse().unwrap_or(IpcContentType::Json),
        target: None,
    };

    let handler = session.stream_registry().get(&request.ipc)
        .ok_or_else(|| format!("unknown streaming ipc: {}", request.ipc))?;

    let ipc_stream = handler.stream(&session, request)
        .map_err(|e| e.to_string())?;

    // Drain receiver into the Tauri channel.
    // Channel::send() is synchronous. Loop blocks until done or cancelled.
    // ConcurrentQueue::pop() blocks until an item is available.
    loop {
        match ipc_stream.receiver.0.pop() {
            Ok(Ok(chunk)) => {
                stream.send(chunk).map_err(|_| "channel closed".to_string())?;
            }
            Ok(Err(e)) => {
                let _ = stream.send(IpcStreamChunk {
                    data: e.to_string().into_bytes(),
                    sequence: u64::MAX,
                    progress: None,
                });
                return Err(e.to_string());
            }
            Err(_) => {
                // Queue closed — all producers dropped.
                return Ok(());
            }
        }
    }
    // When we return, the Rust Channel drops → Tauri sends { end: true } to JS
}

/// Client → server: streaming input.
#[tauri::command]
async fn __ewe_ipc_stream_accept(
    app_handle: tauri::AppHandle,
    ipc: String,
    action: String,
    payload: Vec<u8>,
    content_type: String,
    input: tauri::ipc::Channel<IpcStreamChunk>,
) -> Result<Vec<u8>, String> {
    let session = app_handle.state::<PlatformSession>();
    let request = IpcRequest {
        ipc, action, payload,
        content_type: content_type.parse().unwrap_or(IpcContentType::Json),
        target: None,
    };

    let handler = session.stream_registry().get(&request.ipc)
        .ok_or_else(|| format!("unknown streaming ipc: {}", request.ipc))?;

    // Collect chunks from the input channel into a ConcurrentQueue.
    let queue = concurrent_queue::ConcurrentQueue::unbounded();
    // See design note below about client→server streaming limitations.

    let response = handler.accept_stream(&session, request, IpcStreamReceiver::Sync(queue))
        .map_err(|e| e.to_string())?;

    Ok(response.payload)
}
```

**Important design note for `accept_stream`**: Tauri's `Channel<T>` is
designed for server→client data flow. Client→server streaming (JS pushing
chunks to Rust) is not directly supported by Tauri's Channel API. The
`accept_stream` direction requires either:

1. **Multiple `invoke()` calls**: each chunk is a separate `invoke()` call
   to `__ewe_ipc_stream_accept_chunk`, with a session ID to correlate chunks.
2. **A reverse Channel**: Rust creates a `Channel<T>` and passes its ID to
   JS via the initial response. JS then calls `channel.send()` from its side.
   Tauri supports this on desktop but mobile support varies.

The feature implements approach 1 (multiple invoke calls) as the MVP, with
approach 2 (reverse channel) as a future enhancement gated on Tauri platform
support verification.

### Cancellation

**Server → client**: JS drops the `invokeIpcStream` handle → underlying
`Channel` object gets garbage collected → unregisterCallback fires →
Rust `Channel::send()` returns `Err` → the loop breaks.

**Client → server**: JS stops calling `invoke()` with new chunks. The
`IpcStreamReceiver` times out after a configurable idle period.

### No tokio

Tauri's `Channel::send()` is synchronous (`channel.rs:292-298`). It calls
`on_message` directly — no async, no tokio. The `async` on `#[tauri::command]
async fn __ewe_ipc_stream` is a Tauri framework requirement for all commands
(they're spawned on `tauri::async_runtime`), but our handler code inside
the function body uses `concurrent_queue::ConcurrentQueue` (lock-free, wasm32-safe,
already a workspace dep). This is valtron-compatible.

For truly async streaming (e.g., proxying a WebSocket), `IpcStreamReceiver::Async`
uses valtron primitives (not tokio) behind an `async` feature flag.

## Requirements

### 1. Research and documentation
- Document Tauri Channel internals in `foundations/tauri-channels.md` (NEW)
- Wire format (`"__CHANNEL__:ID"`), `from_callback_fn`, drop signaling, mobile `CHANNELS` map
- Delivery: eval (small) vs fetch (large) vs JNI (mobile)
- Client→server streaming limitations and workarounds

### 2. `StreamingIpc` trait — `foundation_platform`
- File: `backends/foundation_platform/src/ipc/streaming.rs` (NEW)
- Extends `Ipc` (F25). Not an `IpcKind` variant.
- `stream()`: server → client. Returns `IpcStream`.
- `accept_stream()`: client → server. Accepts `IpcStreamReceiver`, returns `IpcResponse`.
- `Ipc::kind()` returns `IpcKind::Query`.
- `IpcStreamChunk`: `data`, `sequence`, `progress` — NO `is_last` (Tauri Channel drop handles it).
- `IpcStreamChunk` implements `Serialize` for `Channel::send()`.
- `IpcStreamReceiver::Sync` via `concurrent_queue::ConcurrentQueue` (lock-free, wasm32-safe). No tokio.

### 3. Tauri commands
- `__ewe_ipc_stream(channel: Channel<IpcStreamChunk>)` — server → client
- Channel deserialized from `"__CHANNEL__:ID"` wire format in the JSON payload
- Drains `IpcStreamReceiver` into the Tauri Channel in a sync loop
- On return: Rust Channel drops → Tauri signals `{ end: true }` to JS
- Handles cancellation (channel close → `send()` returns `Err`)

### 4. Client→server approach (MVP)
- Each chunk is a separate `__ewe_ipc` invoke with `ipc: "filesystem", action: "write_chunk"`
- Session ID (UUID) correlates chunks to a single upload
- Final chunk signals completion; handler returns `IpcResponse`
- Alternative (future): reverse Channel if Tauri platform support confirmed

### 5. `PlatformStreamRegistry`
- Separate from `IpcRegistry` (F25)
- `register()`, `get()`, `names()` on `PlatformSession`
- Handler can be in BOTH registries if it implements `Ipc` + `StreamingIpc`

### 6. JS API (pure JS, no TypeScript)
- `invokeIpcStream(name, action, payload)` → stream handle (server → client)
  - Handle exposes: `onChunk(cb)`, `onDone(cb)`, `onError(cb)`, `cancel()`
- `createUploadStream(name, action)` → upload handle (client → server)
  - Handle exposes: `send(chunk)`, `close()`, `result` (Promise)
- Injected via ScriptInjector (F24) as part of `ipc-bridge.js`
- Uses Tauri `Channel` class internally for wire transport

### 7. Concrete streaming IPCs
- `FileReaderIpc`: `stream()` reads file in 4KB chunks via `ConcurrentQueue::unbounded()`
- MVP client→server: chunked upload via multiple `invoke()` calls

### 8. Testing
- Unit: `IpcStreamReceiver` send/recv, early close (drop sender)
- Unit: `IpcStreamChunk` serialization → `InvokeResponseBody::Json`
- Integration: FileReader IPC streams known file, verifies all chunks in order
- Android: Chrome DevTools shows ordered `onmessage` callbacks

## Verification

```bash
# Core types compile
cargo build -p foundation_platform

# Streaming IPC unit tests
cargo test -p foundation_platform -- ipc_streaming

# FileReader IPC integration
cargo test -p foundation_platform -- file_reader_streaming

# Android: stream a file — ordered chunks arrive
# Chrome DevTools → Console:
# > var s = invokeIpcStream('filesystem', 'read', { path: 'app/index.html' });
# > s.onChunk(function(c) { console.log('chunk ' + c.sequence + ': ' + c.data.length + 'B'); });
# // chunk 0: 4096B
# // chunk 1: 4096B
# // chunk 2: 123B
# // onDone fired  (Tauri Channel drop signaled end)

# Android: upload a file in chunks
# > var u = createUploadStream('filesystem', 'write', { path: '/tmp/out.bin' });
# > u.send(new Uint8Array([1,2,3]));
# > u.send(new Uint8Array([4,5,6]));
# > u.close();
# > u.result.then(function(r) { console.log(r); });  // { written: 6, hash: 'sha256:...' }
```

## Files

| File | Action |
|------|--------|
| `foundations/tauri-channels.md` | **NEW** — Tauri Channel internals: wire format, delivery paths, mobile |
| `backends/foundation_platform/src/ipc/streaming.rs` | **NEW** — `StreamingIpc` trait, `IpcStream`, `IpcStreamReceiver`, `IpcStreamChunk` |
| `backends/foundation_platform/src/ipc/mod.rs` | Add `pub mod streaming;` |
| `backends/foundation_platform/src/ipc/stream_command.rs` | **NEW** — `__ewe_ipc_stream` Tauri command |
| `backends/foundation_platform/src/ipc/stream_registry.rs` | **NEW** — `PlatformStreamRegistry` |
| `backends/foundation_wasm_ui/runtimes/ipc-bridge.js` | Add `invokeIpcStream` + `createUploadStream` |
| `examples/platform_android/src-tauri/src/lib.rs` | Register `FileReaderIpc`, add streaming demo |
