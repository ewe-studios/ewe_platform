# 009 — No backpressure concern (single batch + streaming)

**Date:** 2026-06-08
**Status:** Resolved

### Decision

**No special backpressure mechanism needed.** The architecture naturally prevents accumulation:

### WASM → JS
- Effects queue into `InstructionReceiver` during `stabilize()`
- After stabilize completes: **one protocol-encoded batch** → FFI call
- JS processes ops sequentially from the buffer (zero-copy, no accumulation)

### Server → JS
- Streamed via SSE/WebSocket
- Natural backpressure: consumer processes events at its own pace

### Why no OOM
- Effects don't accumulate pending work — they execute during stabilize, then batch is flushed
- Arrow deserialization is zero-copy — JS creates TypedArray views over the buffer, no intermediate copies
- Streaming from server means consumer controls the rate
