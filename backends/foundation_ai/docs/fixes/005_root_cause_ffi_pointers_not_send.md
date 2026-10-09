# 002 — llama.cpp FFI pointers are !Send

## Symptom

After fixing `Rc<RefCell<T>>` → `Arc<Mutex<T>>`, compilation still fails:

```
`NonNull<infrastructure_llama_bindings::llama_context>` cannot be sent between threads safely
`*mut infrastructure_llama_bindings::llama_sampler` cannot be sent between threads safely
```

## Root Cause

`LlamaCppStreamInner` holds two FFI types that contain raw C pointers:

- `LlamaModelContext<'static>` — wraps `NonNull<llama_context>` (the inference
  context from llama.cpp's C API)
- `LlamaSampler` — wraps `*mut llama_sampler` (the token sampler)

Raw pointers (`*mut T`, `NonNull<T>`) do not implement `Send` because the
compiler cannot verify thread-safety of the underlying C code. Wrapping them
in `Arc<Mutex<T>>` makes the Rust wrapper Send-shaped, but the inner types
still block the auto-trait.

## Why It's Safe

Both FFI pointers are only accessed inside `Iterator::next()`, which runs on
the thread that polls the stream. The stream is created, polled, and dropped
on the same thread — no cross-thread transfer occurs. The `Arc<Mutex<>>` is
for type-level compatibility with the valtron executor, not for actual
concurrent access. The `Mutex` ensures exclusive access regardless.

The `LlamaModelContext` is created lazily inside `next()` (not in the
constructor), so it's always born on the polling thread.

## Fix

Added `unsafe impl Send for LlamaCppStreamInner {}` with a safety comment
documenting the invariant that FFI pointers are only accessed on the polling
thread.

## Files Changed

- `backends/foundation_ai/src/backends/llamacpp.rs`

## Risk

If someone later changes `LlamaCppStream` to be polled from multiple threads
or transfers the inner state across threads, the `unsafe impl Send` would
mask a real soundness bug. The `Mutex` prevents data races at the Rust level,
but the C library may have thread-affinity requirements (e.g. GPU contexts).
The safety comment documents this constraint.
