# Bug Analysis: ByteBufferPointer & WebSocket Frame Decode Failures

**Date:** 2026-03-11
**Affected files:**
- `backends/foundation_core/src/io/ioutils/mod.rs` (`ByteBufferPointer`)
- `backends/foundation_core/src/wire/websocket/frame.rs` (`WebSocketFrame::decode`)
- `backends/foundation_core/src/wire/websocket/task.rs` (`WebSocketTask` state machine)

**Reproducing test:** `test_message_iterator` in `tests/backends/foundation_core/integrations/websocket/echo_tests.rs`

---

## Summary

Three related bugs prevented the WebSocket client from reliably receiving multiple
consecutive frames over a TCP stream. The test sends 3 text messages ("msg1", "msg2",
"msg3") and expects all 3 echoes back. Before the fixes, the test either hung
indefinitely or failed with `InvalidFrame("unknown opcode: 0xD")` on the third frame.

---

## Bug 1: `Ok(0)` from TCP stream treated as EOF

**Location:** `frame.rs` — `WebSocketFrame::decode()`

**Symptom:** WebSocket connection closed prematurely when a TCP `read()` returned
`Ok(0)` during frame header read.

**Root cause:** The original code treated `Ok(0)` from `reader.read()` as
`UnexpectedEof`, which caused the WebSocket task to transition to `Closed` state.
On a TCP stream, `Ok(0)` does not necessarily mean EOF — the stream may simply have
no data available yet.

**Fix:** Changed `Ok(0)` to return `WouldBlock` instead, signaling the caller to retry
on the next poll cycle:

```rust
// frame.rs — WebSocketFrame::decode()
Ok(0) => {
    return Err(WebSocketError::IoError(std::io::Error::new(
        std::io::ErrorKind::WouldBlock,
        "zero bytes read from stream, retry later",
    )));
}
```

---

## Bug 2: `ByteBufferPointer::nextby()` and `peekby()` checked total buffer length instead of unconsumed data

**Location:** `ioutils/mod.rs` — `ByteBufferPointer::nextby()` (~line 1696) and `peekby()` (~line 1643)

**Symptom:** Test hung indefinitely after the HTTP upgrade handshake. The WebSocket task
entered an infinite `Pending` loop because `fill_up()` was never called — `nextby()`
believed it already had enough data.

**Root cause:** The loop condition compared the requested `size` against the total
`buffer.len()`, but after the HTTP handshake consumed buffer data, `peek_pos` had
advanced to the end. The unconsumed data was 0 bytes, yet `buffer.len()` was still ~200
(the full handshake response). The calculation `rem = size - buffer.len()` was negative,
so the loop broke immediately without calling `fill_up()` to read WebSocket frame data.

**Example with numbers:**
```
buffer.len() = 200 (full HTTP response still in buffer)
peek_pos     = 200 (all consumed by handshake parsing)
size         = 1   (reading first header byte)

BEFORE (buggy):
  rem = 1 - 200 = -199  →  rem < 0  →  break (no fill_up!)
  available data = 0 bytes, but code thinks there's 200 bytes

AFTER (fixed):
  available = 200 - 200 = 0
  rem = 1 - 0 = 1  →  rem > 0  →  call fill_up()
```

**Fix:** Changed both `nextby()` and `peekby()` to compute available data as
`buffer.len() - peek_pos`:

```rust
// BEFORE:
let buffer_len = self.buffer.len() as isize;
let rem = (size as isize) - buffer_len;

// AFTER:
let available = (self.buffer.len() as isize) - (self.peek_pos as isize);
let rem = (size as isize) - available;
```

---

## Bug 3: `nextby()` loop break condition `rem < 0` vs `rem <= 0`

**Location:** `ioutils/mod.rs` — `ByteBufferPointer::nextby()` (~line 1699)

**Symptom:** After Bug 2 was fixed, frames 1 and 2 were received successfully, but
frame 3 failed with `InvalidFrame("unknown opcode: 0xD")`.

**Root cause:** When exactly enough data was available (`rem == 0`), the condition
`rem < 0` was false, so the loop entered `fill_up()` unnecessarily. `fill_up()` first
calls `truncate()` which compacts the buffer (moving remaining data to position 0),
then calls `self.reader.read()` on the TCP socket. Since the server had already sent
all data, no more bytes were available, and the read blocked for the full 2-second
timeout.

After the timeout, `fill_up()` returned a `WouldBlock`/`TimedOut` I/O error. Since
the first header byte had already been consumed from the stream, the frame decoder
was now in a corrupt state — on retry it would read payload bytes as the next frame
header, producing garbage opcodes.

**Detailed trace of the corruption:**
```
Frame 3 data in buffer: [0x81, 0x04, 'm', 's', 'g', '3']
                          ^^^^  ^^^^  ^^^^^^^^^^^^^^^^^^^^^^^^
                          hdr1  hdr2  payload (4 bytes)

1. read(&mut header[..1])  — reads 0x81 (first header byte consumed)
2. read_exact(&mut header[1..2]) — reads 0x04 (second header byte consumed)
3. read_exact(&mut payload) — calls nextby(4)
   - available = 4, rem = 4 - 4 = 0
   - rem < 0 is FALSE → enters fill_up()
   - fill_up() truncates buffer to ['m','s','g','3'] then reads TCP...
   - TCP read times out after 2s → WouldBlock error
   - Header bytes already consumed → stream corrupted
4. On retry: next read sees 'm' (0x6D) as first byte → opcode 0xD → InvalidFrame
```

**Fix:** Changed `rem < 0` to `rem <= 0` in the `nextby()` loop:

```rust
// BEFORE:
if rem < 0 {
    break;
}

// AFTER:
if rem <= 0 {
    break;
}
```

This prevents `fill_up()` from being called when exactly the right amount of data
is already available in the buffer.

---

## Bug 3a: Partial frame read safety (defense in depth)

**Location:** `frame.rs` — `WebSocketFrame::decode()`

**Symptom:** When a timeout occurred after partial frame header consumption, the task
retried frame decoding from scratch, reading stale payload bytes as the next header.

**Root cause:** Once the first header byte is consumed from the stream, any subsequent
I/O error (WouldBlock, TimedOut) leaves the stream in an inconsistent state. The
WebSocket task's error handler for WouldBlock/TimedOut returned `Delayed(20ms)` and
retried, but the stream cursor had moved — the next decode attempt read payload bytes
as a frame header.

**Fix:** Added a `map_partial_read_err` closure in `WebSocketFrame::decode()` that
converts WouldBlock/TimedOut errors occurring after the first header byte into
`ProtocolError`. This ensures the task transitions to `Closed` state instead of
retrying with a corrupted stream:

```rust
let map_partial_read_err = |e: WebSocketError| -> WebSocketError {
    match &e {
        WebSocketError::IoError(io_err)
            if io_err.kind() == std::io::ErrorKind::WouldBlock
                || io_err.kind() == std::io::ErrorKind::TimedOut =>
        {
            WebSocketError::ProtocolError(format!(
                "Partial frame read interrupted by I/O error (stream corrupted): {}",
                io_err
            ))
        }
        _ => e,
    }
};

// Applied to all read_exact calls after the first header byte:
reader.read_exact(&mut header[1..2]).map_err(|e| map_partial_read_err(e.into()))?;
reader.read_exact(&mut buf).map_err(|e| map_partial_read_err(e.into()))?;  // extended length
reader.read_exact(&mut key).map_err(|e| map_partial_read_err(e.into()))?;  // mask key
reader.read_exact(&mut payload).map_err(|e| map_partial_read_err(e.into()))?;
```

---

## Interaction between the bugs

The three bugs are layered. Bug 1 was independent. Bug 2 masked Bug 3:

```
Bug 2 (wrong available calc) → test hangs (never reads frame data)
    ↓ fix Bug 2
Bug 3 (rem < 0 vs <= 0) → unnecessary fill_up() → TCP timeout → stream corruption
    ↓ fix Bug 3
All frames read correctly, no unnecessary I/O
```

Bug 3a (partial read safety) is defense-in-depth: even with Bug 3 fixed, any future
scenario where a timeout interrupts a mid-frame read will now produce a clean
`ProtocolError` and connection close, rather than silent stream corruption.

---

## Data flow diagram

```
 TCP Socket
     │
     ▼
 BufReader<BufWriter<Connection>>    (std::io::BufReader)
     │
     ▼
 ByteBufferPointer                   (ioutils/mod.rs)
  ├── buffer: Vec<u8>                  Internal byte buffer
  ├── pos: usize                       Read cursor (committed reads)
  ├── peek_pos: usize                  Peek cursor (tentative reads)
  ├── fill_up()                        Reads from TCP into buffer
  ├── truncate()                       Compacts consumed bytes
  ├── nextby(size)                     Returns `size` bytes, advances peek_pos
  └── read_size(buf)                   nextby() + skip() (commits the read)
     │
     ▼
 SharedByteBufferStream<RawStream>   (Arc<RwLock<ByteBufferPointer>>)
  └── impl Read::read()              Delegates to read_size()
     │
     ▼
 WebSocketFrame::decode(reader)      (frame.rs)
  ├── read() for 1st header byte       Can return WouldBlock (retry-safe)
  ├── read_exact() for 2nd header byte After this, stream is committed
  ├── read_exact() for ext. length
  ├── read_exact() for mask key
  └── read_exact() for payload
     │
     ▼
 WebSocketTask (Open state)          (task.rs)
  ├── WouldBlock/TimedOut → Delayed(20ms), retry
  ├── UnexpectedEof → Closed
  └── ProtocolError → Closed (no retry)
```

---

## Verification

After all fixes applied:
- `test_message_iterator`: **PASS** (35ms, all 3 messages received)
- All 9 WebSocket echo tests: **PASS** (0.10s total)
- All 246 `foundation_core` lib tests: **PASS** (5.13s)
