# 013 — Web Component: 3 Execution Modes (Transport Abstraction)

**Date:** 2026-06-08
**Status:** Resolved

### Decision

**Components and renderer don't care about execution mode.** The transport is handled entirely by the `#[wasm_bin]` vs `#[wasm_worker]` vs `#[wasm_service]` proc macros.

### How it works

```rust
// Component/Renderer code — same regardless of mode
let renderer = ctx.renderer();
renderer.render(html! { ... });
```

When operations need to happen, the **engine** around the WASM binary handles delivery. The WASM binary itself doesn't know where it runs.

### Three modes, same import contract

All three modes call the same WASM imported function. The difference is HOW:

| Mode | How the import is called |
|------|-------------------------|
| `#[wasm_bin]` | Direct call via WASM imports — `wasmExports.invoke(params)` |
| `#[wasm_worker]` | `postMessage` wraps the import call — main thread ↔ worker |
| `#[wasm_service]` | `fetch` wraps the import call — service worker intercepts HTTP |

### Protocol in the message

Every message starts with a protocol byte + version:

```
[protocol: u8][version: u8][payload...]
```

- Protocol byte tells the receiver what format follows (custom binary vs Arrow vs JSON)
- WASM binary doesn't care who sent it — it just reads the protocol byte and dispatches
- The wrapper (proc macro-generated) knows how to deliver the message to the WASM import

### What this means

- **No Transport trait** — proc macros already abstract this
- **No component awareness of mode** — `ctx.renderer()` doesn't know or care
- **No FRAME_BATCH transport coupling** — batcher hands bytes off, wrapper delivers
