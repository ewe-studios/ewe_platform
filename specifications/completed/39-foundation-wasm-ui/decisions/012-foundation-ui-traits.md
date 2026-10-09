# 012 — `foundation_ui_traits` crate: shared types between signals and UI

**Date:** 2026-06-08
**Status:** Resolved

### Decision

Create `foundation_ui_traits` crate — owned by `foundation_wasm_ui` but a dependency of **both** `foundation_signals` and `foundation_wasm_ui`:

```
                    foundation_ui_traits
                    ├── IntoHtml trait
                    ├── Html struct
                    ├── Part descriptors
                    └── DomOp enum
                     ↑          ↑
                     |          |
           foundation_signals  foundation_wasm_ui
```

### Why

- `foundation_signals` needs `IntoHtml` to impl on `Signal<T>` — can't depend on `foundation_wasm_ui` (circular)
- `foundation_ui_traits` is the shared middle layer — lightweight, no runtime dependencies
- `foundation_wasm_ui` implements the trait logic (DOM ops, Arrow encoding)
- `foundation_signals` just uses the trait as a constraint

### What lives here

- `IntoHtml` trait definition
- `Html` struct (lightweight tree: tag, attrs, children, Parts)
- `Part` descriptors (TextPart, AttrPart, EventPart, ChildPart)
- `DomOp` enum (maps to Arrow operations)
- `ProtocolEncoder<T>` trait — pure encoding contract (`T → Vec<u8>`, `&[u8] → DecodeResult`)
- `ArrowEncoder` — `Vec<DomOp>` → Arrow IPC bytes
- `JsonEncoder` — `Vec<DomOp>` → JSON bytes
- `CustomBinaryEncoder` — `Vec<DomOp>` → custom binary bytes
- `Envelope` — 14-byte message header write/parse (pure bytes, no WASM types)

The encoders live here because they operate on `DomOp` (which is defined here) and produce pure `Vec<u8>` with no WASM, FFI, or allocator dependencies. This makes them usable by HTTP servers, WebSocket servers, SSE endpoints, CLI tools — anything that needs to produce protocol-encoded bytes without a WASM runtime.

### What does NOT live here

- Signal types (in `foundation_signals`)
- WASM transport / arena memory (in `foundation_wasm`)
- WASM protocol impls that compose encoding + transport (in `foundation_wasm_ui`)
- Runtime, Context, Effects (in `foundation_signals`)
