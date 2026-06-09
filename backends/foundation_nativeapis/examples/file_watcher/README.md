# Example: File Watcher

## Purpose

Demonstrates the native file watching layer (`native_watcher`) for monitoring directory changes. Shows how to set up a watcher that reports file creation, modification, deletion, and rename events.

## Prerequisites

- Feature flags: `watcher-linux` (includes `poll`)
- Linux with inotify, or macOS with kqueue

## How to Run

```bash
# Start the watcher on a directory
cargo run -p foundation_nativeapis --features watcher-linux --example file_watcher /tmp/watched-dir

# In another terminal, trigger events:
mkdir -p /tmp/watched-dir
echo "hello" > /tmp/watched-dir/test.txt
echo "world" >> /tmp/watched-dir/test.txt
rm /tmp/watched-dir/test.txt
```

## Architecture

The example:

1. Creates a native file watcher via `native_watcher()`
2. Watches the provided directory (and subdirectories if `-r`)
3. Polls for events in a loop, printing each event
4. Handles `Created`, `Modified`, `Removed`, and `Renamed` event types

The watcher is built on top of the same poll infrastructure as `RegisteredFd`, using platform-native mechanisms (inotify on Linux, kqueue on macOS).

## Expected Output

```
Watching /tmp/watched-dir for changes...
Events: Created, Modified, Removed, Renamed
(Ctrl+C to exit)
Event: Created /tmp/watched-dir/test.txt
Event: Modified /tmp/watched-dir/test.txt
Event: Removed /tmp/watched-dir/test.txt
```

## Key APIs Demonstrated

- `native_watcher()` — creates the platform-appropriate watcher
- `Watcher::watch()` — add a path with optional recursive monitoring
- `Watcher::poll()` — collect pending events

## Related

- Feature spec: `specifications/37-overlay-vfs/features/10-valtron-integration/feature.md`
- Source: `src/native/watcher/`
