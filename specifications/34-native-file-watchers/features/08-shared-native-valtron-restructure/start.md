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

1. **One-way dependency**: `native/` → `shared/`, `valtron/` → `shared/`, `valtron/native/` → `native::fd`. Never the reverse.
2. **`shared/` is always compiled** — no `#[cfg(feature)]` gates on the module itself
3. **Crate-root re-exports** — all types available at `foundation_nativeapis::X` for backward compatibility
4. **`valtron/` split into shared + native** — only `FdMonitorTask` needs `native::fd`; the rest (`broadcaster`, `file_watcher`, `stop_signal`) are purely shared
5. **`FdMonitorTask` uses `TaskStatus::Depends`** — executor parks task until OS signals fd readiness; no wasted periodic polling
6. **`watcher` feature removed** — `PollWatcher` always available; platform watchers individually gated
7. **`default = ["task", "native"]`** — sensible out-of-the-box behavior; platform-specific watchers opt-in via `native-linux`, etc.

---

_Created: 2026-06-03_
