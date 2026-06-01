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

## Architecture

### Core Types

```rust
/// Join a message bus. Returns (sender, receiver) pair.
pub fn join<T: MessageBox, R: MessageBox>(
    options: Options,
    timeout: Option<Duration>,
) -> Result<(EndpointSender<T>, EndpointReceiver<R>)>;

/// Options for joining a bus.
pub struct Options {
    /// Bus identifier (unique name for the bus).
    pub identifier: String,
    /// Endpoint label (for routing).
    pub label: Label,
    /// Authentication token (optional).
    pub token: String,
    /// Whether to become the bus controller if none exists.
    pub controller_affinity: bool,
}
```

### Message Model

```rust
/// A message with typed payload.
pub struct Message<T> {
    pub selector: Selector,       // routing: unicast/multicast + label matching
    pub payload: T,                // typed payload
    pub objects: Vec<Object>,      // kernel objects (FD/Handle/MachPort)
    pub memory_regions: Vec<MemoryRegion>, // zero-copy shared memory blocks
}

/// Routing rules for messages.
pub struct Selector {
    pub label_op: LabelOp,         // AND/OR/NOT label matching
    pub mode: SelectorMode,        // Unicast or Multicast
    pub ttl: Duration,             // time-to-live if unroutable
}

/// Label-based routing with logical operations.
pub enum LabelOp {
    True, False,
    Leaf(String),
    Not(Box<LabelOp>),
    And(Box<LabelOp>, Box<LabelOp>),
    Or(Box<LabelOp>, Box<LabelOp>),
}
```

### Object Passing

```rust
/// Platform-native kernel object.
#[cfg(target_os = "linux")]  pub type Object = Fd;       // raw fd
#[cfg(target_os = "macos")]  pub type Object = MachPort; // mach_port_t
#[cfg(target_os = "windows")] pub type Object = Handle;  // HANDLE

// Sending an object to another endpoint:
let mut message = Message::new(selector, payload);
message.objects.push(unsafe { Object::from_raw(inotify_fd) });
sender.send(message)?;

// Receiving endpoint gets the object with ownership.
```

### Shared Memory (MemoryRegion)

```rust
/// Zero-copy shared memory block.
/// Linux: memfd_create + mmap
/// macOS: vm_allocate + vm_map
/// Windows: CreateFileMapping + MapViewOfFile
pub struct MemoryRegion {
    // platform-specific internal
}

impl MemoryRegion {
    pub fn new(size: usize) -> Option<Self>;
    pub fn map(&mut self, range: impl RangeBounds<usize>) -> &mut [u8];
    pub fn buffer_size(&self) -> u64;
}

// Send large data without copying:
let mut region = MemoryRegion::new(1 << 20)?;  // 1MB
let view = region.map(..);
view.copy_from_slice(&large_data);
message.memory_regions.push(region);
sender.send(message)?;
```

### Bus Controller

The first endpoint to join a bus with `controller_affinity: true` becomes the bus controller:
- Listens for new connections on the bus identifier
- Routes messages based on selector label matching
- Handles object/memory region forwarding between endpoints
- If controller drops, remaining endpoints auto-rejoin

### IO Multiplexing

Uses the same epoll/kqueue/IOCP patterns from our extracted poll layer:
- Linux: `epoll_create1` + `eventfd` waker (same as `IoMultiplexing` in ipmb)
- macOS: kqueue + `EVFILT_USER` waker
- Windows: IOCP + overlapped I/O

### FFI Layer

Same pattern as ipmb-ffi — opaque types, `extern "C"` functions:

```c
// ipmb.h (generated)
typedef struct ipmb_Sender* ipmb_Sender;
typedef struct ipmb_Receiver* ipmb_Receiver;
typedef struct ipmb_Message* ipmb_Message;

int32_t ipmb_join(ipmb_Options options, uint32_t timeout_ms,
                  ipmb_Sender* out_sender, ipmb_Receiver* out_receiver);
int32_t ipmb_send(ipmb_Sender sender, ipmb_Message message);
int32_t ipmb_recv(ipmb_Receiver receiver, ipmb_Message* out_message, uint32_t timeout_ms);
```

## How valtron Uses It

```rust
// Task A sends file change notifications to other processes
let (sender, _) = ipmb::join::<FileEvent, FileEvent>(
    Options::new("com.ewe.watchers", label!("file-watcher"), ""),
    None,
)?;

// On file change, broadcast to all listeners
let selector = Selector::multicast(LabelOp::True);
let message = Message::new(selector, FileEvent { path, kind });
sender.send(message)?;

// Task B in another process receives events
let (_, mut receiver) = ipmb::join::<FileEvent, FileEvent>(
    Options::new("com.ewe.watchers", label!("build-system"), ""),
    None,
)?;

while let Ok(msg) = receiver.recv(None) {
    rebuild(msg.payload.path);
}
```

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

---

_Created: 2026-06-01_
