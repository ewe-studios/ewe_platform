# foundation_nostd — no_std primitives

## What it is
Core primitives that work in `no_std` environments (embedded, wasm, bare-metal):
allocators, atomics, synchronization, and utility types.

## Key modules
- **`primitives/`** — Atomic cells, flags, barriers, condvars, mutexes, rwlocks,
  once cells, spin waits. All work without OS support.
- **`comp/`** — Compression utilities that work in `no_std`.
- **`alloc/`** — Custom allocators for constrained environments.

## Design principles
- **no_std first**: Everything compiles without `std`. The `std` feature adds
  convenience wrappers and OS-backed implementations.
- **wasm compatible**: Primitives work on `wasm32-unknown-unknown` where there
  are no threads or OS primitives.
- **cooperative**: Spin waits are cooperative — they yield to the executor on
  each iteration, preventing busy-waiting on single-threaded targets.

## When to use
Use `foundation_nostd` when:
- Building for embedded/wasm without `std`
- Need fine-grained control over synchronization behavior
- Want primitives that work identically across all targets

Use `foundation_core`'s sync layer when you need higher-level primitives
(channels, waitgroups) that build on top of these.
