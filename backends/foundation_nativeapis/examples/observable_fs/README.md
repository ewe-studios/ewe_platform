# Example: ObservableFs

## Purpose

Demonstrates the event-driven VFS pattern using `ObservableFs` — a wrapper around any `VfsFileSystem` implementation that emits a stream of `VfsEvent` notifications for every filesystem operation. This example shows how to subscribe to filesystem events, interpret the different event types, and build reactive systems that respond to file changes in real time.

## Prerequisites

- Rust toolchain with workspace dependencies
- Feature flag: `vfs`

## How to Run

```bash
cargo run -p foundation_nativeapis --features vfs --example observable_fs
```

## Architecture

`ObservableFs` is a decorator that wraps an inner `VfsFileSystem` (in this case, `MemoryFs`) and intercepts every mutating operation to emit a `VfsEvent`:

1. **Subscriber model** — `subscribe()` returns an `mpsc::Receiver<VfsEvent>`. Multiple subscribers can be created, each receiving an independent event stream.

2. **Event emission** — After each operation (create, write, rename, remove, etc.), the wrapper sends a structured event containing:
   - Event type (Created, Written, Renamed, Removed, etc.)
   - Affected path(s)
   - Metadata where relevant (e.g., bytes written, new/old paths for rename)

3. **Non-blocking by default** — Events are buffered; if no subscriber is listening, they are simply dropped (no backpressure on the VFS operations).

The example performs a sequence of filesystem operations and then drains the event queue to show what was captured.

## Expected Output

```
=== ObservableFs Example ===

Created /data
Created and wrote to /data/log.txt
Wrote /data/config.json
Stat'd /data/log.txt
Renamed config.json → settings.json

--- Events received ---
  Created { path: "/data", file_type: Directory }
  Created { path: "/data/log.txt", file_type: File }
  Written { path: "/data/log.txt", bytes: 11 }
  Created { path: "/data/config.json", file_type: File }
  Written { path: "/data/config.json", bytes: 17 }
  Stat { path: "/data/log.txt" }
  Renamed { old_path: "/data/config.json", new_path: "/data/settings.json" }

Total events: 7

=== Done ===
```

## Key APIs Demonstrated

- `ObservableFs::new(inner)` — Wrap any `VfsFileSystem` to make it observable
- `ObservableFs::subscribe()` — Create a new event subscriber (`mpsc::Receiver<VfsEvent>`)
- `VfsEvent::Created` — Emitted when a file or directory is created
- `VfsEvent::Written` — Emitted when data is written to a file, includes byte count
- `VfsEvent::Renamed` — Emitted on rename, includes both old and new paths
- `VfsEvent::Removed` — Emitted when a file or directory is deleted
- `VfsEvent::Stat` — Emitted on stat/metadata queries (read-only observation)
- `mpsc::Receiver::recv()` — Blocking receive of the next event from the stream

## Where to Use This

- **File watchers** — Implement `inotify`/`fsevents`-like functionality for hot-reload
- **Audit logging** — Record every filesystem mutation for compliance and debugging
- **Build system invalidation** — Trigger recompilation when source files change
- **Sync engines** — Detect local changes and propagate them to remote replicas
- **IDE integration** — Update project views, linting, and type-checking on file edits
- **Testing assertions** — Verify that a function produces the expected sequence of filesystem operations

## Related

- Feature spec: `specifications/37-overlay-vfs/features/03-observable-fs/feature.md`
- Source: `src/shared/vfs/observable_fs.rs`
- Trait definition: `src/shared/vfs/traits.rs` (`VfsFileSystem`, `VfsEvent`)
