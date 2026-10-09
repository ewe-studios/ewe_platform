# 07 — Native integration: hybrid webview + native, native shell + WASM

**Date:** 2026-07-04
**Status:** Resolved

### Decision

Hybrid by design. The JS DOM applicator remains the primary rendering surface
for web content. Native UI (Swift/Kotlin) is additive, not a replacement. The
platform also supports a native shell that embeds a WASM runtime for
zero-copy Arrow communication between Rust and native code in the same process.

### Native UI integration

Three tiers of native integration:

1. **Use Tauri's built-in capabilities** where they meet our needs — window
   management, native menus, platform plugins, filesystem APIs.

2. **Build native Swift/Kotlin bridges** where Tauri falls short — native
   navigation controllers, tab bars, sheets, biometric flows, platform-specific
   media APIs, background services, hardware integration.

3. **Sync between web and native** — the platform lets Rust/JS content drive
   native UI state (e.g., update a native tab badge from a server stream) and
   lets native UI events feed back into the web runtime (e.g., a native back
   gesture routing through the session's navigation policy).

Nothing is wrong with writing native code when it adds value. Hotwire Native's
ability to sync into native navigation stacks, tab bars, and platform chrome
is a net benefit that `foundation_platform` embraces.

### Native shell + WASM

A thin, pre-compiled static library that embeds a WASM runtime (wasmtime,
wasm3, or wasmi) serves as an intermediary between native platform code and
user-provided WASM modules:

```
Native app (Swift/Kotlin/C++)
  │ same process, shared memory
  ▼
Native Shell (compiled static library, shipped once)
  ├── Embedded WASM runtime
  └── Arrow buffers in WASM linear memory
  │ loads and executes
  ▼
User WASM module (may also run in the WebView, or only on the backend)
```

**Zero-copy Arrow across WASM ↔ native.** The WASM module writes Arrow
`RecordBatch`es into its linear memory. The native shell reads those bytes
directly (same process, same address space). Arrow's columnar layout IS the
in-memory format — no serialization, no copy.

**Two tiers of native integration, both with zero-copy:**

| Tier | What the user ships | Zero-copy? | App-store update? |
|------|---------------------|------------|-------------------|
| Static library | Native `.a`/`.so` of Rust logic | Yes (same process) | Full review for logic changes |
| Shell + WASM | Native shell (once) + WASM module | Yes (WASM linear memory) | WASM hot-swap, no review |

**Shell-owned update lifecycle.** The shell can fetch the latest WASM modules,
frontend shell assets (HTML, CSS, JS, images), and static resources from an
update service; validate integrity; hot-swap without restart; roll back on
failure; manage the cache. The entire presentation layer updates on the fly
without app-store rebuild.

**User WASM runs anywhere.** The WebView embeds `foundation-wasm-ui`'s own
WASM-based runtime. The user's application WASM can run in the native shell,
in an IPC process, on a local server, or on a remote server. Wherever it runs,
it streams responses to the runtime in the WebView. Users CAN also deploy
their WASM to the frontend for local execution — it's an architectural
choice, not a platform constraint.

### Rust↔native zero-copy (same process)

On all platforms, Rust compiled as a static library runs in the same process as
the application. No sandbox boundary, no serialization. A Rust function
allocates an Arrow `RecordBatch`, returns a pointer + length, and the caller
reads the exact same bytes. This works today via UniFFI (iOS), JNI (Android),
and direct FFI (desktop).

Arrow's standardized columnar memory layout means the wire format IS the
in-memory format. A 10MB table crosses the Rust→Swift boundary in
microseconds — the cost of a pointer write, not a memory copy.
