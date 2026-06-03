---
workspace_name: "ewe_platform"
spec_directory: "specifications/34-native-file-watchers"
feature_directory: "specifications/34-native-file-watchers/features/08-shared-native-valtron-restructure"
this_file: "specifications/34-native-file-watchers/features/08-shared-native-valtron-restructure/start.md"
created: 2026-06-03
---

# Start: Shared / Native / Valtron Module Restructure

## Agent Workflow

1. Read `feature.md` (detailed requirements + tasks + critical rules)
2. Read `../../LEARNINGS.md` (past discoveries and mistakes)
3. Work through tasks in order, updating checkbox status as you complete them
4. **ALWAYS UPDATE ../../LEARNINGS.md** after each completed task/milestone

---

**Critical Rules Summary:**

1. **One-way dependency**: `native/` → `shared/`, `valtron/` → `shared/`, `valtron/` → `native::fd` (for FdMonitorTask). Never the reverse.
2. **`shared/` is always compiled** — no `#[cfg(feature)]` gates on the module itself, only on platform-specific sub-items within `native/`
3. **Crate-root re-exports** — all types available at `foundation_nativeapis::X` for backward compatibility
4. **Valtron tasks are shareable** — they don't belong in `native/`. `StopSignal`, `EventBroadcaster` are generic over `EventReadiness`, not tied to any OS API
5. **`watcher` feature removed** — `PollWatcher` is always available; platform-specific watchers are individually gated (`watcher-linux`, `watcher-macos`, `watcher-windows`)

---

_Created: 2026-06-03_
