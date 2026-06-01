---
feature: "Interprocess Message Bus (IPC)"
description: "Cross-platform IPC bus with typed messaging, shared memory (MemoryRegion), kernel object passing (FD/Handle/MachPort), and FFI bindings — adapted from ipmb"
status: "pending"
priority: "medium"
depends_on: ["01-native-apis"]
estimated_effort: "large"
created: 2026-06-01
last_updated: 2026-06-01
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 14
  total: 14
  completion_percentage: 0%
---

# Feature: Interprocess Message Bus (IPC)

## Problem

valtron tasks and foundation components need to communicate across processes — not just within a single runtime. The existing approach would require setting up separate networking, handling serialization, managing connections manually.

ipmb from Bytedance provides a clean bus-based IPC model:
- No server/client distinction — any endpoint can join and communicate freely
- Typed messages with selectors (unicast/multicast + label matching)
- Zero-copy shared memory (`MemoryRegion`) for large payloads
- Kernel object passing (FDs on Linux, Handles on Windows, MachPorts on macOS)
- Built-in FFI for C/C++ and other languages

## Solution

Adapt ipmb's architecture into `foundation_nativeapis` as an IPC module. The bus runs on native transports:

| Platform | Transport | Mechanism |
|----------|-----------|-----------|
| Linux | Unix domain sockets (`SOCK_SEQPACKET`) | Abstract socket addresses, `SCM_RIGHTS` for FD passing |
| macOS | Mach ports | `mach_msg`, `mach_port` operations |
| Windows | Named pipes | `CreateNamedPipe`, `ConnectNamedPipe` |

---

## Component 1: Core Message Bus

### What It Does

Provides a bus-based IPC where endpoints join a named bus, send typed messages, and receive messages that match their label selectors. Think of it like a pub/sub system where the "bus" is the transport layer, not a central server.

### How It Works — Connection Flow

```
Process A                              Process B
    │                                      │
    │  join("com.ewe.watchers",             │  join("com.ewe.watchers",
    │       label!("file-watcher"))              label!("build-system"))
    │                                      │
    ▼                                      ▼
┌─────────────────┐                  ┌─────────────────┐
│ 1. Create socket│                  │ 1. Create socket│
│ 2. Connect to   │                  │ 2. Connect to   │
│    bus address   │                  │    bus address   │
│ 3. Send JOIN msg│ ───────────────► │                 │
│                 │                  │                 │
│                 │ ◄─────────────── │ 3. Send JOIN msg│
└─────────────────┘                  └─────────────────┘
    │                                      │
    │  controller endpoint                 │  controller endpoint
    │  (first with affinity)               │  (first with affinity)
    │                                      │
    ▼                                      ▼
┌─────────────────┐                  ┌─────────────────┐
│ sender.send(msg)│ ──► controller ─►│ receiver.recv() │
│                 │    routes by     │                 │
│                 │    label match   │                 │
└─────────────────┘                  └─────────────────┘
```

### Core API

```rust
/// Join a message bus. Returns (sender, receiver) pair.
pub fn join<T: MessageBox, R: MessageBox>(
    options: Options,
    timeout: Option<Duration>,
) -> Result<(EndpointSender<T>, EndpointReceiver<R>)>;
```

### Options

```rust
pub struct Options {
    /// Bus identifier — unique name all endpoints on the same bus share.
    /// e.g., "com.ewe.watchers", "com.ewe.build-system"
    pub identifier: String,
    /// Endpoint label — used for routing. Messages delivered only to matching labels.
    pub label: Label,
    /// Authentication token — optional. If set, endpoints must share the same token.
    pub token: String,
    /// Whether to become the bus controller if none exists.
    pub controller_affinity: bool,
}
```

---

## Component 2: Message Model

### Message Structure

```rust
/// A message with typed payload and optional kernel objects / shared memory.
pub struct Message<T> {
    pub selector: Selector,              // routing: unicast/multicast + label matching
    pub payload: T,                      // typed payload, serializable via bincode
    pub objects: Vec<Object>,            // kernel objects (FD/Handle/MachPort)
    pub memory_regions: Vec<MemoryRegion>, // zero-copy shared memory blocks
}
```

### Routing Selectors

```rust
pub struct Selector {
    pub label_op: LabelOp,     // AND/OR/NOT label matching
    pub mode: SelectorMode,    // Unicast or Multicast
    pub ttl: Duration,         // time-to-live if unroutable
}

impl Selector {
    pub fn broadcast() -> Self;                           // send to all
    pub fn unicast(label: &str) -> Self;                  // send to specific endpoint
    pub fn multicast(label_op: LabelOp) -> Self;          // send to all matching
}

#[derive(Clone, PartialEq)]
pub enum SelectorMode {
    Unicast,    // delivers to first matching endpoint
    Multicast,  // delivers to all matching endpoints
}
```

### Label-Based Routing

```rust
/// A label is a string identifier for an endpoint.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Label(String);

/// Logical expression for label matching.
pub enum LabelOp {
    True,                                    // always matches
    False,                                   // never matches
    Leaf(String),                            // matches specific label
    Not(Box<LabelOp>),                       // negation
    And(Box<LabelOp>, Box<LabelOp>),         // conjunction
    Or(Box<LabelOp>, Box<LabelOp>),          // disjunction
}
```

**How label matching works:**

```rust
fn matches(selector: &Selector, endpoint_label: &Label) -> bool {
    evaluate_label_op(&selector.label_op, &endpoint_label.0)
}

fn evaluate_label_op(op: &LabelOp, label: &str) -> bool {
    match op {
        LabelOp::True => true,
        LabelOp::False => false,
        LabelOp::Leaf(s) => label == s,
        LabelOp::Not(sub) => !evaluate_label_op(sub, label),
        LabelOp::And(left, right) => evaluate_label_op(left, label) && evaluate_label_op(right, label),
        LabelOp::Or(left, right) => evaluate_label_op(left, label) || evaluate_label_op(right, label),
    }
}
```

Usage:
```rust
// Send to all endpoints
Selector::broadcast()

// Send to a specific endpoint
Selector::unicast("build-system")

// Send to endpoints matching complex label expression
Selector::multicast(label!("build-system" | "ci-runner"))

// Send to "build-system" but not "test-only"
Selector::multicast(label!("build-system" & !"test-only"))
```

---

## Component 3: MessageBox Trait + Serialization

### What It Does

`MessageBox` is the trait that makes a type sendable over the IPC bus. It's implemented via a derive macro that generates bincode serialization/deserialization.

```rust
/// Trait for types that can be sent over the IPC bus.
/// Implement via derive macro: #[derive(MessageBox)]
pub trait MessageBox: Serialize + DeserializeOwned + Send + Sync + 'static {
    /// Unique type identifier for dynamic message type resolution.
    fn type_uuid() -> u128;
}

/// Derive macro usage:
/// #[derive(MessageBox, Serialize, Deserialize)]
/// struct FileEvent {
///     path: String,
///     kind: String,
/// }
```

### How Serialization Works

1. Sender calls `sender.send(Message { payload: FileEvent { ... }, ... })`
2. Payload is serialized via bincode into bytes
3. Message header written: `[type_uuid: u128][selector_size: u32][payload_size: u32][object_count: u32]`
4. Payload bytes written
5. Objects attached via ancillary data (SCM_RIGHTS on Linux) or platform equivalent
6. Receiver reads header, deserializes payload based on `type_uuid`
7. Objects extracted from ancillary data and attached to Message

---

## Component 4: Bus Controller

### What It Does

The bus controller is the first endpoint that joins a bus with `controller_affinity: true`. It:
- Listens for new endpoint connections
- Routes incoming messages based on selector label matching
- Forwards messages to matching endpoints
- If the controller drops, remaining endpoints auto-rejoin

### How It Works

```
Controller Thread:
┌──────────────────────────────────────────┐
│ 1. Listen socket accepts new connections │
│ 2. New endpoint sends JOIN message       │
│    → controller stores (conn, label)      │
│ 3. Receive message from any endpoint     │
│ 4. Evaluate selector against all labels  │
│ 5. Forward message to matching endpoints │
│ 6. If endpoint disconnects, remove it    │
│ 7. If TTL expires, drop unrouted message │
└──────────────────────────────────────────┘
```

The controller runs on our `poll::Poll` layer — it polls the listen socket and all endpoint connections simultaneously. When the listen socket is readable, a new connection arrived. When an endpoint socket is readable, a message arrived from that endpoint.

### Auto-Rejoin

If the controller process dies, remaining endpoints detect the disconnection (socket error on recv). They automatically attempt to rejoin:
1. Create new connection to bus identifier
2. Send JOIN message
3. If no controller responds, the endpoint with `controller_affinity: true` becomes the new controller
4. Other endpoints reconnect to the new controller

---

## Component 5: Shared Memory (MemoryRegion)

### What It Does

Zero-copy shared memory blocks for large payloads. Instead of serializing a 1MB buffer and sending it over the socket, create a shared memory region, write to it, and pass the region handle to the receiver.

### How It Works

```rust
pub struct MemoryRegion {
    // platform-specific internal
}

impl MemoryRegion {
    /// Create a new shared memory region of `size` bytes.
    pub fn new(size: usize) -> Option<Self>;

    /// Map a range of the region into the process address space.
    /// Returns a mutable slice — write data directly into shared memory.
    pub fn map(&mut self, range: impl RangeBounds<usize>) -> &mut [u8];

    /// Total size of the region in bytes.
    pub fn buffer_size(&self) -> u64;
}
```

**Platform implementations:**

| Platform | Mechanism |
|----------|-----------|
| Linux | `memfd_create()` → creates anonymous file descriptor → `mmap()` to map into address space → FD passed via `SCM_RIGHTS` |
| macOS | `mmap()` with `MAP_ANON | MAP_SHARED` → `vm_allocate` → region handle passed via Mach port |
| Windows | `CreateFileMapping(INVALID_HANDLE_VALUE, ...)` → `MapViewOfFile()` → handle passed via `DuplicateHandle` |

**Usage:**
```rust
let mut region = MemoryRegion::new(1 << 20)?;  // 1MB
let view = region.map(..);
view.copy_from_slice(&large_data);
message.memory_regions.push(region);
sender.send(message)?;
// Receiver gets a MemoryRegion they can map and read — no copy.
```

---

## Component 6: Object Passing

### What It Does

Pass kernel objects (file descriptors, handles) across process boundaries. The sender puts the object in `message.objects`, the receiver gets ownership.

```rust
/// Platform-native kernel object.
#[cfg(target_os = "linux")]  pub type Object = Fd;       // raw fd
#[cfg(target_os = "macos")]  pub type Object = MachPort; // mach_port_t
#[cfg(target_os = "windows")] pub type Object = Handle;  // HANDLE

impl Object {
    /// Create from a raw value. The object must already be valid.
    pub unsafe fn from_raw(raw: Self::Raw) -> Self;
    /// Consume and return the raw value.
    pub fn into_raw(self) -> Self::Raw;
}
```

**How it works on Linux:**
1. Sender calls `sendmsg()` with `SCM_RIGHTS` ancillary data containing the fd
2. Kernel duplicates the fd into the receiver's fd table
3. Receiver calls `recvmsg()` and extracts fds from `SCM_RIGHTS` ancillary data
4. Receiver gets ownership — original fd in sender remains valid (it's a dup, not a move)

---

## Component 7: IO Multiplexing

### What It Does

Uses our extracted `poll::Poll` layer for efficient IO multiplexing within the bus controller and endpoints.

**Linux:** `epoll_create1` + `eventfd` waker (same pattern as our poll layer)
**macOS:** kqueue + `EVFILT_USER` waker
**Windows:** IOCP + overlapped I/O

The controller polls all endpoint connections simultaneously. This is why we need the poll layer — without it, the controller would need one thread per endpoint, which doesn't scale.

---

## Component 8: FFI Layer

### What It Does

Provides `extern "C"` bindings so C/C++ and other languages can use the IPC bus.

```c
// ipmb.h
typedef struct ipmb_Sender* ipmb_Sender;
typedef struct ipmb_Receiver* ipmb_Receiver;
typedef struct ipmb_Message* ipmb_Message;

int32_t ipmb_join(ipmb_Options options, uint32_t timeout_ms,
                  ipmb_Sender* out_sender, ipmb_Receiver* out_receiver);
int32_t ipmb_send(ipmb_Sender sender, ipmb_Message message);
int32_t ipmb_recv(ipmb_Receiver receiver, ipmb_Message* out_message, uint32_t timeout_ms);
void    ipmb_message_free(ipmb_Message msg);
void    ipmb_sender_free(ipmb_Sender sender);
void    ipmb_receiver_free(ipmb_Receiver receiver);
```

**Rust side:**
```rust
#[no_mangle]
pub extern "C" fn ipmb_join(
    options: ipmb_Options,
    timeout_ms: u32,
    out_sender: *mut *mut ipmb_Sender,
    out_receiver: *mut *mut ipmb_Receiver,
) -> i32 {
    // Creates opaque Rust types, wraps in Box, returns raw pointers
}
```

---

## Component 9: Version Compatibility Protocol

### What It Does

Every message carries a version header. Endpoints refuse to communicate with incompatible versions, preventing silent data corruption from protocol changes.

### How It Works

**Version format:** `[magic: u8 = 0xFF][major: u8][minor: u8][patch: u8]` — packed as `u32`.

**Compatibility rule:**
- If both major versions are `0` → minor versions must match (pre-1.0 breaking changes allowed on minor bump)
- If either major version is `≥1` → major versions must match (semver rules)

```rust
#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Version((u8, u8, u8));

impl Version {
    fn compatible(&self, rhs: Self) -> bool {
        if self.major() == 0 && rhs.major() == 0 {
            self.minor() == rhs.minor()
        } else {
            self.major() == rhs.major()
        }
    }

    pub fn major(&self) -> u8 { self.0 .0 }
    pub fn minor(&self) -> u8 { self.0 .1 }
    pub fn patch(&self) -> u8 { self.0 .2 }
}

/// Version read from crate metadata at compile time
static VERSION: Lazy<Version> = Lazy::new(|| {
    let v_major = env!("CARGO_PKG_VERSION_MAJOR");
    let v_minor = env!("CARGO_PKG_VERSION_MINOR");
    let v_patch = env!("CARGO_PKG_VERSION_PATCH");
    Version((v_major.parse().unwrap(), v_minor.parse().unwrap(), v_patch.parse().unwrap()))
});

pub fn version() -> Version { *VERSION }
```

**Where version is checked:**
1. **On join (connect handshake)**: Endpoint sends version in `ConnectMessage`. Controller compares and responds with `ConnectMessageAck::Ok` or `ErrVersion`.
2. **On receive**: `EncodedMessage::from_local()` / `EncodedMessage::new()` checks version in every received message. `VersionMismatch` returned if incompatible.

---

## Component 10: ConnectMessage Handshake

### What It Does

When an endpoint joins a bus, it doesn't just connect to a socket — it performs a handshake to verify version, token, and register its label with the controller.

### How It Works

**Step-by-step (ipmb/src/platform/linux.rs `look_up()`):**

```
1. Endpoint creates SOCK_SEQPACKET socket
2. Connects to abstract socket address (bus identifier)
3. Creates socketpair for local communication
4. Sends ConnectMessage with:
   - version (own version)
   - token (auth token)
   - label (routing label)
   - write end of socketpair as object (for controller's reply)
5. Waits for ConnectMessageAck on the socketpair read end
6. Controller receives ConnectMessage, validates:
   a. version.compatible(own_version)? → if no, send ErrVersion(own_version)
   b. token == own_token? → if no, send ErrToken
   c. If valid: create EndpointID (UUID v4), store (label, remote) in endpoints list
      → send Ok(endpoint_id)
7. Endpoint receives ack → if Ok, creates IoHub with the reply fd, returns (IoHub, Remote, EndpointID)
```

**Message types:**

```rust
#[derive(Debug, Serialize, Deserialize, TypeUuid)]
#[uuid = "b2c1deb3-3091-4a74-a99c-c8e8d710d4b2"]
pub struct ConnectMessage {
    pub version: Version,
    pub token: String,
    pub label: Label,
}

#[derive(Debug, Serialize, Deserialize, TypeUuid)]
#[uuid = "c3de9eb4-c310-4c14-9747-093d62c09998"]
pub enum ConnectMessageAck {
    Ok(EndpointID),
    ErrVersion(Version),
    ErrToken,
}
```

---

## Component 11: EncodedMessage Wire Format

### What It Does

Messages are encoded as raw bytes for transmission. The wire format carries version, selector, and payload in a single buffer with 4-byte alignment padding.

### Wire Format

```
┌─────────────────────────────────────────────────────────┐
│ version: u32 (0xFF | major | minor | patch)            │ 4 bytes
├─────────────────────────────────────────────────────────┤
│ selector_size: u32                                      │ 4 bytes
├─────────────────────────────────────────────────────────┤
│ selector: bincode(serialized Selector)                  │ selector_size bytes
├─────────────────────────────────────────────────────────┤
│ selector_padding (align to 4 bytes)                     │ 0-3 bytes
├─────────────────────────────────────────────────────────┤
│ payload_size: u32                                       │ 4 bytes
├─────────────────────────────────────────────────────────┤
│ payload: bincode(serialized T)                          │ payload_size bytes
├─────────────────────────────────────────────────────────┤
│ payload_padding (align to 4 bytes)                      │ 0-3 bytes
└─────────────────────────────────────────────────────────┘

Objects/MemoryRegions: sent via ancillary data (SCM_RIGHTS on Linux)
or Mach port descriptors (macOS) or handle duplication (Windows).
```

**Linux encoding (ipmb/src/platform/linux.rs `Message::encode_inner()`):**
- `iov_data`: version + selector_size + selector + padding + payload_size + payload
- `control_data`: `cmsghdr` with `SOL_SOCKET`/`SCM_RIGHTS` containing fd array
- `sendmsg()` with iov + control data

**macOS encoding:** Same layout but embedded in `mach_msg_header_t` + `mach_msg_body_t` + `mach_msg_port_descriptor_t` array.

---

## Component 12: Message Buffer (TTL Retry)

### What It Does

When the controller can't route a message (no matching endpoint), it buffers the message with an expiration time. When a new endpoint connects, the controller retries all buffered messages.

### How It Works (ipmb/src/bus_controller.rs)

```rust
// In handle_message():
if !routed && encoded_msg.selector.label_op.validate(&self.label) {
    // Controller itself is the target
    match self.sender.send(encoded_msg) {
        Ok(_) => {}
        Err(err) => {
            if !routed {
                remain = Some(err.0);  // message couldn't be sent
            }
        }
    }
} else {
    if !routed {
        remain = Some(encoded_msg);  // no matching endpoint found
    }
}

// If there's an unrouted message and it has TTL:
if let Some(remain) = remain {
    if !remain.selector.ttl.is_zero() {
        self.message_buffer.push((now + remain.selector.ttl, remain));
    }
}

// When a new endpoint connects (endpoint_connected == true):
if endpoint_connected && !self.message_buffer.is_empty() {
    let mut message_buffer = mem::take(&mut self.message_buffer);
    for (expire, msg) in message_buffer.drain(..) {
        let (remain, _) = self.handle_message(msg);
        if let Some(remain) = remain {
            if expire > now {  // still valid
                self.message_buffer_swap.push((expire, remain));
            }
        }
    }
    mem::swap(&mut self.message_buffer, &mut self.message_buffer_swap);
}

// Periodic cleanup:
fn maintain(&mut self, now: Instant) {
    self.message_buffer.retain(|(expire, _)| *expire > now);
}
```

---

## Component 13: Endpoint Reachability Detection

### What It Does

The controller periodically checks if connected endpoints are still alive. Dead endpoints are removed from the endpoint list to prevent routing to disconnected peers.

### How It Works (ipmb/src/bus_controller.rs)

```rust
fn detect_reachable(&mut self, now: Instant) {
    if now - self.last_detect_reachable > Duration::from_secs(30) {
        self.endpoints.retain(|ep| !ep.remote.is_dead());
        self.last_detect_reachable = now;
    }
}
```

**`Remote::is_dead()` per platform:**
- **Linux**: Implicit — `sendmsg()` returns error, `EncodedMessage::from_local()` returns `Error::Disconnect`
- **macOS**: `mach_port_type()` → check `MACH_PORT_TYPE_DEAD_NAME` flag
- **Windows**: `WriteFile()` with 0 bytes — returns false if pipe broken

---

## Component 14: Auto-Rejoin with Epoch Tracking

### What It Does

When an endpoint loses connection to the controller (controller crash, network partition), it automatically re-joins with epoch tracking to avoid stale state.

### How It Works (ipmb/src/lib.rs `Rule::join()`)

```rust
enum Rule {
    Client {
        endpoint_id: EndpointID,
        options: Options,
        remote: Remote,
        io_hub: Option<Mutex<IoHub>>,
        reader_closed: bool,
        im: Arc<IoMultiplexing>,
        epoch: u32,  // incremented on each re-join
    },
    Server {
        endpoint_id: EndpointID,
        bus_sender: Mutex<Sender<EncodedMessage>>,
        receiver: Option<Mutex<Receiver<EncodedMessage>>>,
        im: Arc<IoMultiplexing>,
    },
}

// On send/recv Error::Disconnect:
let epoch = *epoch;
drop(rule);

let mut rule = self.rule.write().unwrap();
match &mut *rule {
    Rule::Client { options, io_hub, reader_closed, im, epoch: epoch1, .. } => {
        if epoch == *epoch1 {
            let reader_closed = *reader_closed;
            drop(io_hub.take());  // close old connection

            *rule = Rule::join(
                options.clone(),
                epoch.overflowing_add(1).0,  // epoch + 1
                im.clone(),
                None,  // re-join with no timeout (blocking)
            )?;

            if reader_closed {
                rule.reader_close();
            }
        }
    }
    Rule::Server { .. } => {}
}
```

**Join retry loop:** The `Rule::join()` function loops with 2-second backoff, retrying up to 5 times for `PermissionDenied` and timeout errors.

---

## Component 15: Error Type Hierarchy

### What It Does

Four error types distinguish between internal errors and user-facing errors at different lifecycle stages.

```rust
/// Internal error — used throughout the IPC layer
#[derive(Debug, Error)]
pub enum Error {
    #[error("encode error")]
    Encode(#[from] bincode::error::EncodeError),
    #[error("decode error")]
    Decode(#[from] bincode::error::DecodeError),
    #[error("type uuid not found")]
    TypeUuidNotFound,
    #[error("timeout")]
    Timeout,
    #[error("disconnected")]
    Disconnect,
    #[error("version mismatch: {0}")]
    VersionMismatch(Version, Option<Remote>),
    #[error("token mismatch")]
    TokenMismatch,
    #[error("identifier in use")]
    IdentifierInUse,
    #[error("identifier not in use")]
    IdentifierNotInUse,
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("memory region mapping error")]
    MemoryRegionMapping,
    #[error("permission denied")]
    PermissionDenied,
    #[error("unknown error")]
    Unknown,
}

/// Error from join() — endpoint couldn't connect to bus
#[derive(Debug, Error)]
pub enum JoinError {
    #[error("version mismatch: {0}")]
    VersionMismatch(Version),
    #[error("token mismatch")]
    TokenMismatch,
    #[error("timeout")]
    Timeout,
    #[error("permission denied")]
    PermissionDenied,
}

/// Error from send() — message couldn't be sent
#[derive(Debug, Error)]
pub enum SendError {
    #[error("timeout")]
    Timeout,
    #[error("version mismatch: {0}")]
    VersionMismatch(Version),
    #[error("token mismatch")]
    TokenMismatch,
    #[error("permission denied")]
    PermissionDenied,
}

/// Error from recv() — message couldn't be received
#[derive(Debug, Error)]
pub enum RecvError {
    #[error("decode error")]
    Decode(#[from] bincode::error::DecodeError),
    #[error("timeout")]
    Timeout,
    #[error("version mismatch: {0}")]
    VersionMismatch(Version),
    #[error("token mismatch")]
    TokenMismatch,
    #[error("permission denied")]
    PermissionDenied,
}

// Conversions: JoinError → SendError, JoinError → RecvError
impl From<JoinError> for SendError { ... }
impl From<JoinError> for RecvError { ... }
```

---

## Additional Details

### EndpointSender / EndpointReceiver Semantics

- **`EndpointSender<T>` implements `Clone`** — can be shared across threads. All clones send to the same bus.
- **`EndpointReceiver<R>` does NOT implement `Clone`** — each receiver owns its receiving kernel buffer. Dropping the receiver closes the buffer.

### BytesMessage (Built-in Raw Bytes Type)

```rust
#[derive(Debug, Serialize, Deserialize, TypeUuid)]
#[uuid = "dd95ba8e-1279-47cf-925e-83e614e79588"]
pub struct BytesMessage {
    pub format: u16,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}
```

Useful for forwarding arbitrary data without defining a custom type. The `format` field lets the receiver interpret the raw bytes.

### MemoryRegion Reference Counting

`MemoryRegion` uses atomic reference counting stored in the shared memory header:

```rust
// Header layout in shared memory:
// [reference_count: AtomicU32 (4 bytes)][buffer_size: u64 (8 bytes)][buffer data...]

pub(crate) fn ref_count_inner(&self, val: i32) -> u32 {
    let rc: &AtomicU32 = unsafe { mem::transmute(self.header.as_slice().as_ptr()) };
    if val == 0 { rc.load(Ordering::SeqCst) }
    else if val > 0 { rc.fetch_add(val as _, Ordering::SeqCst) }
    else { rc.fetch_sub(-val as _, Ordering::SeqCst) }
}
```

- `new()`: ref count = 1
- `clone()`/send: ref count += 1 (before sendmsg)
- Receive: ref count -= 1 (after mapping)
- `Drop`: ref count -= 1

### MessageBox Derive Macro Constraints

The `#[derive(MessageBox)]` macro **only supports enums with unnamed single fields** per variant:

```rust
// VALID:
#[derive(MessageBox, Serialize, Deserialize)]
pub enum MyMessage {
    Text(String),
    Data(Vec<u8>),
    Event(FileEvent),
}

// INVALID:
#[derive(MessageBox)]  // panic! — struct, not enum
pub struct MyMessage { ... }

#[derive(MessageBox)]  // panic! — multiple fields per variant
pub enum MyMessage {
    Text(String, u32),
}

#[derive(MessageBox)]  // panic! — named field
pub enum MyMessage {
    Text { content: String },
}
```

### EndpointID

```rust
#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct EndpointID(Bytes);  // 16 bytes

impl EndpointID {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().into_bytes())
    }
}
```

Unique identifier for each endpoint, assigned by the controller on successful connection.

### Align4 Utility

All sizes in the wire format are padded to 4-byte boundaries:

```rust
pub trait Align4 {
    fn align4(self) -> Self;
}

impl Align4 for usize {
    fn align4(mut self) -> Self {
        if (self & 0x3) != 0 { self = (self & !0x3) + 4; }
        self
    }
}
```

---

## Reference Source (for implementation)

| Component | ipmb source file |
|-----------|-----------------|
| Main lib (join, EndpointSender/Receiver, Rule, Version) | `ipmb/src/lib.rs` |
| Bus controller | `ipmb/src/bus_controller.rs` |
| Error types | `ipmb/src/errors.rs` |
| Options | `ipmb/src/options.rs` |
| Message types (MessageBox, ConnectMessage, BytesMessage) | `ipmb/src/message.rs` |
| Label / LabelOp | `ipmb/src/label.rs` |
| MemoryRegistry | `ipmb/src/memory_registry.rs` |
| Platform module (Object, MemoryRegion, look_up, register) | `ipmb/src/platform/mod.rs` |
| Linux: socket, IoHub, EncodedMessage, IoMultiplexing | `ipmb/src/platform/linux.rs` |
| Linux: EncodedMessage (wire format, send/recv) | `ipmb/src/platform/linux/encoded_message.rs` |
| Linux: IoMultiplexing (epoll + eventfd) | `ipmb/src/platform/linux/io_mul.rs` |
| Linux: Fd, Local, Remote | `ipmb/src/platform/linux/fd.rs` |
| macOS: mach ports, bootstrap, IoMultiplexing | `ipmb/src/platform/macos/mod.rs` |
| macOS: mach_sys bindings | `ipmb/src/platform/macos/mach_sys.rs` |
| macOS: MemoryRegion | `ipmb/src/platform/macos/memory_region.rs` |
| Windows: named pipes, IoHub | `ipmb/src/platform/windows/mod.rs` |
| Windows: MemoryRegion | `ipmb/src/platform/windows/memory_region.rs` |
| Windows: pipe handling | `ipmb/src/platform/windows/pipe.rs` |
| Windows: security attributes | `ipmb/src/platform/windows/security.rs` |
| Windows: utilities | `ipmb/src/platform/windows/util.rs` |
| Derive macro | `ipmb-derive/src/lib.rs` |
| FFI C++ wrapper | `ipmb-ffi/src/lib.rs` |
| Utility (Align4, EndpointID, range_to_offset_size) | `ipmb/src/util/mod.rs` |

---

## Testing Strategy

### What to Test

#### Unit Tests (in `src/`)

1. **Label matching**: All `LabelOp` combinations evaluate correctly
   ```rust
   assert!(evaluate_label_op(&LabelOp::Leaf("foo".into()), "foo"));
   assert!(!evaluate_label_op(&LabelOp::Leaf("foo".into()), "bar"));
   assert!(evaluate_label_op(&LabelOp::Or(Box::new(Leaf("a")), Box::new(Leaf("b"))), "b"));
   assert!(evaluate_label_op(&LabelOp::Not(Box::new(Leaf("a"))), "b"));
   ```

2. **Selector construction**: `broadcast()`, `unicast()`, `multicast()` create correct selectors

3. **MessageBox derive**: Generated serialization/deserialization round-trips correctly

4. **MemoryRegion**: Create, map, write, read — data persists across map calls

5. **Message encoding**: Header + payload + objects encode/decode correctly

#### Integration Tests (in `tests/`)

6. **ipc_two_process.rs**: Two processes join same bus, send typed messages, verify delivery
   - Process A: `join("test-bus", label!("sender"))` → send message
   - Process B: `join("test-bus", label!("receiver"))` → receive message
   - Verify payload matches sent data

7. **ipc_multicast.rs**: Controller routes messages to multiple matching endpoints
   - 3 endpoints join with labels "a", "b", "c"
   - Send multicast to `Or("a", "c")` → only endpoints "a" and "c" receive
   - Send unicast to "b" → only endpoint "b" receives

8. **ipc_controller_failover.rs**: Controller drops, endpoints auto-rejoin
   - Start controller + 2 endpoints
   - Kill controller process
   - Verify endpoints reconnect and messages still flow

9. **ipc_shared_memory.rs**: Large data sent via MemoryRegion, no copy
   - Create 1MB MemoryRegion, write data
   - Send via IPC, receiver maps and reads
   - Verify data matches

10. **ipc_object_passing.rs**: FD passed via IPC, receiver can use it
    - Linux: open a file, put fd in message.objects, send
    - Receiver gets fd, reads from it — same file content

11. **ipc_ffi.rs**: FFI bindings work from C perspective
    - Call `ipmb_join` from test, send message, receive message
    - Verify no memory leaks (valgrind or AddressSanitizer)

12. **ipc_version.rs**: Version compatibility protocol
    - Join with same version → success
    - Join with incompatible version → `JoinError::VersionMismatch`
    - Join with `0.x.y` vs `0.z.y` (different minor) → rejected
    - Join with `1.x.y` vs `1.a.b` (same major) → accepted

13. **ipc_message_buffer.rs**: Message buffer with TTL retry
    - Send message to non-existent label with TTL
    - New endpoint joins with matching label → message delivered
    - Wait for TTL expiry → message dropped from buffer

14. **ipc_bytes_message.rs**: BytesMessage send/receive
    - Send `BytesMessage { format: 1, data: vec![...] }` → receiver decodes correctly

### Edge Cases to Test

- **Bus name collision**: Two controllers try to create the same bus → second becomes endpoint
- **Token mismatch**: Endpoint joins with wrong token → `JoinError::TokenMismatch`
- **Version mismatch**: Endpoint joins with incompatible version → `JoinError::VersionMismatch`
- **Message too large**: Payload exceeds socket buffer → error returned, not panic
- **Controller crash during send**: Sender detects `Error::Disconnect`, re-joins with `epoch + 1`
- **Unicode labels**: Labels with non-ASCII characters → routing still works
- **Empty message**: Send message with no payload, no objects → delivers correctly
- **Concurrent sends**: Multiple threads cloning `EndpointSender` and sending → no data corruption
- **Timeout on recv**: `recv(Some(Duration::ZERO))` returns `RecvError::Timeout` if no message
- **TTL expiration**: Message with short TTL, unroutable for longer → controller drops it
- **Endpoint disconnect mid-broadcast**: Controller sending to 3 endpoints, one disconnects → others still get message
- **Message buffer retry**: Send message to non-existent endpoint with TTL → new endpoint joins → message delivered
- **Reachability detection**: Kill endpoint process → controller detects dead port within 30s → removes from list
- **BytesMessage**: Send raw bytes with format field → receiver decodes correctly
- **MessageBox derive constraint**: Derive on struct → compile error; derive on enum with named fields → compile error
- **MemoryRegion ref count**: Clone MemoryRegion, send, drop → ref count tracked correctly, memory freed at 0
- **4-byte alignment**: Serialize payload with non-aligned size → wire format padded correctly, decode succeeds

### How to Test

```bash
# Unit tests
cargo test -p foundation_nativeapis --lib ipc

# Integration tests (single-process)
cargo test -p foundation_nativeapis --test ipc_multicast
cargo test -p foundation_nativeapis --test ipc_shared_memory
cargo test -p foundation_nativeapis --test ipc_object_passing
cargo test -p foundation_nativeapis --test ipc_ffi
cargo test -p foundation_nativeapis --test ipc_controller_failover

# Cross-platform compilation
cargo check -p foundation_nativeapis --features "ipc"
cargo check -p foundation_nativeapis --features "ipc" --target wasm32-unknown-unknown  # should fail or stub
```

---

## Implementation Plans

### Task Breakdown

#### 1. Core Message Bus (adapted from ipmb)
1. [ ] Create `src/ipc/mod.rs` — `join()`, `Options`, `Selector`, `Label`, `LabelOp`
2. [ ] Create `src/ipc/message.rs` — `Message<T>`, `MessageBox` derive macro, encoding/decoding
3. [ ] Create `src/ipc/bus_controller.rs` — bus controller with message routing
4. [ ] Create `src/ipc/memory_registry.rs` — pooled shared memory allocation

#### 2. Platform Transports
5. [ ] Create `src/ipc/platform/mod.rs` — `Object`, `MemoryRegion`, `EncodedMessage`
6. [ ] Create `src/ipc/platform/linux.rs` — Unix domain sockets (`SOCK_SEQPACKET`), abstract sockets, `SCM_RIGHTS`
7. [ ] Create `src/ipc/platform/linux/io_mul.rs` — `IoMultiplexing` (epoll + eventfd) — or reuse our poll layer
8. [ ] Create `src/ipc/platform/macos.rs` — Mach ports, `mach_msg`, `vm_allocate`/`vm_map`
9. [ ] Create `src/ipc/platform/windows.rs` — Named pipes, `CreateFileMapping`, `MapViewOfFile`

#### 3. FFI Layer
10. [ ] Create `src/ipc/ffi.rs` — opaque types, `extern "C"` functions (join, send, recv, memory region, objects)
11. [ ] Create `include/ipmb.h` — C header generation or manual

#### 4. Integration
12. [ ] Add `serde` + `bincode` + `type-uuid` dependencies for message serialization
13. [ ] Create `src/ipc/derive.rs` — `MessageBox` derive macro (adapted from `ipmb_derive`)
14. [ ] Write integration test: two processes join same bus, send typed messages, verify delivery

## Trade-offs

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Transport | Unix sockets / Mach ports / Named pipes | Native, fast, kernel-managed. No network overhead. |
| Serialization | bincode + type-uuid | Fast, compact, type-safe. type-uuid enables dynamic message type resolution. |
| Message routing | Label-based with AND/OR/NOT | Flexible, composable, no central registry needed. |
| Controller | First-come becomes controller | Simple, no election protocol needed. Auto-rejoin on drop. |
| FFI | Opaque types + `extern "C"` | Standard C ABI, usable from any language with FFI. |
| Large payloads | MemoryRegion (zero-copy) | Avoid serializing megabytes through socket buffers. |
| Poll layer reuse | Uses our extracted `poll::Poll` | No duplicate IO multiplexing code. Controller uses same selector. |

## File Changes Summary

| File | Action |
|------|--------|
| `backends/foundation_nativeapis/src/ipc/mod.rs` | Create |
| `backends/foundation_nativeapis/src/ipc/message.rs` | Create |
| `backends/foundation_nativeapis/src/ipc/bus_controller.rs` | Create |
| `backends/foundation_nativeapis/src/ipc/memory_registry.rs` | Create |
| `backends/foundation_nativeapis/src/ipc/label.rs` | Create |
| `backends/foundation_nativeapis/src/ipc/errors.rs` | Create |
| `backends/foundation_nativeapis/src/ipc/platform/mod.rs` | Create |
| `backends/foundation_nativeapis/src/ipc/platform/linux.rs` | Create |
| `backends/foundation_nativeapis/src/ipc/platform/linux/io_mul.rs` | Create |
| `backends/foundation_nativeapis/src/ipc/platform/macos.rs` | Create |
| `backends/foundation_nativeapis/src/ipc/platform/windows.rs` | Create |
| `backends/foundation_nativeapis/src/ipc/ffi.rs` | Create |
| `backends/foundation_nativeapis/src/ipc/derive.rs` | Create — `MessageBox` derive macro |
| `backends/foundation_nativeapis/Cargo.toml` | Edit — add serde, bincode, type-uuid deps |
| `backends/foundation_nativeapis/tests/ipc_two_process.rs` | Create — two-process IPC test |
| `backends/foundation_nativeapis/tests/ipc_multicast.rs` | Create — multicast routing test |
| `backends/foundation_nativeapis/tests/ipc_controller_failover.rs` | Create — controller failover test |
| `backends/foundation_nativeapis/tests/ipc_shared_memory.rs` | Create — shared memory test |
| `backends/foundation_nativeapis/tests/ipc_object_passing.rs` | Create — object passing test |
| `backends/foundation_nativeapis/tests/ipc_ffi.rs` | Create — FFI binding test |

---

_Created: 2026-06-01_
