# Example: IPC Bus

## Purpose

Demonstrates the IPC bus capabilities — sending typed messages between processes with various patterns: benchmark, latency measurement, multiple type routing, region-free allocation, rejoining sessions, and triangle routing.

## Prerequisites

- Feature flags: `vfs`
- Linux/macOS

## How to Run

Each example has its own binary:

```bash
# Throughput benchmark
cargo run -p foundation_nativeapis --features vfs --example ipc_bench

# Latency measurement (round-trip)
cargo run -p foundation_nativeapis --features vfs --example ipc_latency

# Multiple type routing over a single connection
cargo run -p foundation_nativeapis --features vfs --example ipc_multiple_type

# Region-free memory allocation
cargo run -p foundation_nativeapis --features vfs --example ipc_region_free

# Rejoining existing IPC sessions
cargo run -p foundation_nativeapis --features vfs --example ipc_rejoin

# Triangle routing (3-process topology)
cargo run -p foundation_nativeapis --features vfs --example ipc_triangle
```

## Architecture

The IPC bus provides typed message passing between processes using shared memory and event notification. The examples demonstrate:

- **ipc_bench**: Measures message throughput between two processes
- **ipc_latency**: Measures round-trip latency for single messages
- **ipc_multiple_type**: Shows routing of different message types over one connection
- **ipc_region_free**: Demonstrates the shared memory allocator without region tracking
- **ipc_rejoin**: Shows how a process can rejoin an existing IPC session
- **ipc_triangle**: Three-process routing topology (A↔B↔C)

## Expected Output

Each example prints timing statistics and message counts.

## Key APIs Demonstrated

- `IpcBus::new()` — create or join an IPC session
- `IpcBus::send()` — send a typed message
- `IpcBus::recv()` — receive pending messages
- `IpcBus::close()` — tear down the session

## Related

- Feature spec: `specifications/37-overlay-vfs/features/11-ipc-daemon/feature.md`
- Source: `src/native/ipc/`
