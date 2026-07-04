# 08 — Arrow positioning and data model

**Date:** 2026-07-04
**Status:** Resolved

### Decision

Arrow IPC is for structured data payloads, NOT for UI DOM operations. The
existing `foundation_wasm_ui` protocols handle UI operations. Arrow is used
for data sets and analytics-style payloads where its columnar layout and
zero-copy guarantees add real value.

### What Arrow is for

- Large structured data payloads (analytics, tables, sync logs).
- High-throughput Rust↔native communication (same process, shared memory).
- Data delivered to the WebView via custom protocol as binary `ArrayBuffer`
  for Arrow JS to parse directly.

### What Arrow is NOT for

- UI DOM operations — those use the existing `DomOp` / columnar v1 / custom
  binary / JSON / HTML fragment protocols from `foundation_wasm_ui`.
- Every efficient batch operation — spec 39 corrected the naming: **columnar
  v1** is the wasm-loop, no-std, TypedArray-friendly layout for DOM operation
  batches. Real Arrow IPC is for the data layer.

### Terminology distinction

- **Columnar v1** — wasm-loop, no-std, typed-array-friendly layout used for
  efficient DOM operation batches. Owned by `foundation_wasm_ui`.
- **Arrow IPC** — real Apache Arrow IPC, owned by `foundation_arrow`, used
  where a std-capable Arrow path is appropriate (data sets, analytics,
  native↔native communication).

### Zero-copy directions

**Rust ↔ Native platform (same process): easy.** No sandbox boundary. Arrow's
wire format IS the in-memory format. Real zero-copy, verifiable — pointer
write, not memory copy. Works on iOS (UniFFI), Android (JNI), desktop (FFI).

**Rust → WebView (cross-process on mobile): harder.** On desktop, native Rust
and WebView are same-process. On mobile, the WebView runs in a separate OS
process — zero-copy is fundamentally limited by sandboxing. The goal is
copy-minimized binary transfer, not literal shared pointers.

**Native shell + WASM (same process): turns the hard case into the easy case.**
The shell as a static library with embedded WASM runtime means WASM linear
memory is in the same process. Arrow data in WASM memory is directly readable
by native code. Zero-copy without static library compilation.

### Concrete copy behavior, ordered by cost

1. **WASM memory as shared buffer** — native Rust writes into WASM linear
   memory; JS reads from the same `ArrayBuffer`. Closest to true zero-copy
   into the WebView. Requires native side managing the WASM instance.
2. **Custom protocol + binary response** — Rust serves Arrow IPC bytes over
   Tauri custom protocol. WebView `fetch()`es it as binary. Single copy
   (Rust buffer → fetch buffer). Recommended default for data payloads.
3. **postMessage + transferable ArrayBuffer** — ownership transfer where
   WebView supports it. Sending context loses access; receiving context gains
   it. Platform-dependent.
4. **SharedArrayBuffer** — true shared memory. Requires COOP/COEP headers.
   Desktop-feasible; mobile support mixed.
5. **Tauri command IPC** — serialization inherent. Control lane only.
6. **WebSocket to local process** — TCP/IP overhead even on localhost. Use
   when the backend is a standalone process already speaking WebSocket.
7. **Disk-based handoff (mmap)** — page sharing on desktop; forced copy on
   mobile. Only for data too large to buffer.
