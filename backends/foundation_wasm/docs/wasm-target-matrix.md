# The Wasm Target Matrix

Four wasm targets with fundamentally different capabilities:

## Target Comparison

| | unknown-unknown | emscripten | wasip1 | wasip2 |
|---|---|---|---|---|
| **std** | No (no libc) | Partial (emscripten libc) | Yes (WASI preview1) | Yes (component model) |
| **Time** | Panics — needs polyfill | std::time works | std::time works | std::time works |
| **Threads** | No | pthreads (optional) | wasi-threads | Yes |
| **Host model** | JS (wasm-bindgen / foundation_wasm abi) | Emscripten + JS | WASI host (wasmtime/wasmer) | WASI component host |
| **Networking** | fetch() via web-sys | emscripten sockets | WASI sockets | wasi:http |
| **Use** | CF Workers, browser SPA | Browser inference (llama, WebGPU) | Server-side wasm, Deno | Component-model deployments |
| **Entropy** | getRandomValues (JS) | getentropy (libc) | random_get (WASI) | wasi:random |

## Canonical cfg Discriminators

```rust
// Native (not wasm at all):
#[cfg(not(target_family = "wasm"))]

// ANY wasm target (all four):
#[cfg(target_family = "wasm")]

// The restrictive no-std target (browser/CF Workers — needs polyfills + JS host):
#[cfg(all(target_family = "wasm", not(target_os = "emscripten"), not(target_os = "wasi")))]

// Emscripten (libc + threads + WebGPU; llama works):
#[cfg(all(target_family = "wasm", target_os = "emscripten"))]

// WASI (preview1 + preview2):
#[cfg(all(target_family = "wasm", target_os = "wasi"))]

// Targets where std is available (native + emscripten + WASI):
#[cfg(any(not(target_family = "wasm"), target_os = "emscripten", target_os = "wasi"))]

// Targets where std is NOT available (unknown-unknown only):
#[cfg(all(target_family = "wasm", not(target_os = "emscripten"), not(target_os = "wasi")))]
```

## Migration Rule

The spec migrated from `target_arch = "wasm32"` to `target_family = "wasm"` (covers wasm32
+ wasm64). Use `target_family`, not `target_arch`.

Where the distinction matters (C tooling, std availability, host model), add `target_os`
qualification:

- **Native-only C tooling** (fff's heed/git2/memmap2, llama's CMake): gate to
  `not(target_family = "wasm")` or `any(not(target_family = "wasm"), target_os = "emscripten")`
  depending on whether it builds under emscripten's libc.
- **Pure-Rust agentic machinery** (foundation_compact, foundation_vectors, agentic types/loop):
  target ALL four.
- **JS host ABI** (wasm_import_module, schedule_timeout, host_apply): gate behind the `web`
  feature, not target checks — it's a host choice, not a target choice.

## Emscripten

Emscripten provides a POSIX-like libc on top of WebAssembly. Key properties:
- `std::time::Instant` works (emscripten_get_now)
- `std::thread::spawn` works with pthreads (SharedArrayBuffer required in browser)
- `std::fs` works via the virtual filesystem (MEMFS/IDBFS)
- CMake toolchains work → llama.cpp, candle builds are possible
- WebGPU access via emscripten bindings

Build: `cargo build --target wasm32-unknown-emscripten` (requires EMSDK; vendored at
`tools/emsdk`). Run: node or browser.

## WASI (WebAssembly System Interface)

### Preview 1 (wasip1)

WASI preview1 defines a syscall-style interface:
- `fd_read`/`fd_write` — file I/O
- `clock_time_get` — monotonic/realtime clocks → `std::time` works
- `random_get` — cryptographic entropy → `getrandom` works
- `environ_get`/`args_get` — environment/args

Rust target: `wasm32-wasip1`. Runtime: wasmtime, wasmer, Deno (`--allow-read` etc.).

### Preview 2 (wasip2) — Component Model

WASI 0.2 uses the Component Model (WIT interfaces):
- `wasi:clocks/monotonic-clock` — timing
- `wasi:random/random` — entropy
- `wasi:http/outgoing-handler` — outbound HTTP
- `wasi:io/streams` — streaming I/O

Rust target: `wasm32-wasip2`. Requires `cargo-component` or `wit-bindgen` for WIT bindings.

## foundation_wasm Host Features

| Feature | Host | Target |
|---------|------|--------|
| `web` | JS ABI (wasm_import_module = "abi") | unknown-unknown, emscripten |
| `wasi` | WASI preview1 syscalls | wasip1 |
| `wasip2` | WASI 0.2 component imports | wasip2 (implies `wasi`) |

On unknown-unknown WITHOUT the `web` feature, the core machinery (memory, encoding,
registries, operations) still compiles — only the host interaction layer is missing.

## Building and Testing Each Target

```bash
# unknown-unknown (default wasm):
cargo check -p foundation_ai --target wasm32-unknown-unknown

# wasip1:
rustup target add wasm32-wasip1
cargo check -p foundation_ai --target wasm32-wasip1

# wasip2:
rustup target add wasm32-wasip2
cargo check -p foundation_ai --target wasm32-wasip2

# emscripten (requires EMSDK):
# source tools/emsdk/emsdk_env.sh
# cargo build --target wasm32-unknown-emscripten -p foundation_compact

# native (always works):
cargo check -p foundation_ai
```
