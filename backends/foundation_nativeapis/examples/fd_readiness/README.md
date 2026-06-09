# Example: FD Readiness

## Purpose

Demonstrates how to use `RegisteredFd` with the poll selector to monitor an arbitrary file descriptor for readability. This is the building block that `FdMonitorTask` uses under the hood for async readiness tracking.

## Prerequisites

- Feature flags: `fd` (includes `poll`)
- Linux/macOS (unix-like with epoll/kqueue)

## How to Run

```bash
cargo run -p foundation_nativeapis --features fd --example fd_readiness
```

## Architecture

The example creates a pipe pair (`reader` → `writer`), registers the reader FD with the poll selector for readability, and then:

1. Shows the initial poll returns "not ready" (pipe is empty)
2. Writes data to the pipe writer
3. Shows the poll now returns "ready" (data available to read)
4. Reads the data and clears the readiness

This demonstrates the core pattern used throughout the I/O layer: register → poll → react.

## Expected Output

```
=== FD Readiness Example ===
Registered fd for read interest
Initial poll: not ready (pipe is empty)
Wrote 13 bytes to pipe
Poll after write: ready! (data available)
Read: "hello world"
Cleared readiness state
Done
```

## Key APIs Demonstrated

- `Poll::new()` — creates the platform selector (epoll/kqueue)
- `Registry::register()` — registers an fd with interest
- `RegisteredFd::with_interest()` — RAII fd registration
- `Poll::poll()` — waits for readiness events

## Related

- Feature spec: `specifications/37-overlay-vfs/features/23-inode-native-vfs/feature.md`
- Source: `src/native/fd/mod.rs`, `src/native/poll/mod.rs`
