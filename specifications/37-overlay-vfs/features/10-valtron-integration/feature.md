---
feature_name: "Valtron Integration"
description: "VfsTask — valtron task that consumes ObservableFs events via Broadcaster, integrating with spec-34 file watcher infrastructure for multi-subscriber event consumption and reactive workflows."
status: "pending"
priority: "medium"
phase: 5
created: 2026-06-04
updated: 2026-06-04
dependencies:
  - "01-core-traits"
  - "15-observable-fs"
tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---

# Feature 10: Valtron Integration

## Overview

Bridges ObservableFs with the valtron task execution model. VfsTask is a valtron task that consumes the event stream from ObservableFs and makes it available through the valtron task infrastructure. Follows the same pattern as `FileWatcherTask` from spec-34.

ObservableFs (feature 15) handles event emission as a separate concern. This feature wires those events into valtron for reactive workflows (incremental builds, live reload, change-driven pipelines).

## Tasks

### VfsTask (`src/valtron/vfs_task.rs`)

- [ ] Define `VfsTask` struct: holds event receiver from ObservableFs Broadcaster
- [ ] Implement valtron `TaskIterator` for `VfsTask`: polls event stream, yields VfsEvents
- [ ] Implement `VfsTask::from_observable(observable_fs)` — subscribe to ObservableFs and wrap
- [ ] Follows same pattern as `FileWatcherTask` from spec-34

### Tests

- [ ] Test: write through ObservableFs, VfsTask yields corresponding event
- [ ] Test: VfsTask integrates with valtron executor (spawn, poll, receive)

## Verification

- Tests pass
- VfsTask works within valtron executor lifecycle
- Integrates with existing Broadcaster from foundation_nativeapis
