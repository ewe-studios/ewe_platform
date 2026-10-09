---
workspace_name: "ewe_platform"
spec_directory: "specifications/34-native-file-watchers"
feature_directory: "specifications/34-native-file-watchers/features/07-valtron-task-design"
this_file: "specifications/34-native-file-watchers/features/07-valtron-task-design/start.md"
created: 2026-06-03
---

# Start: Valtron Task Design Feature

## Agent Workflow

1. Read `feature.md` (detailed requirements + tasks + critical rules)
2. **Read the Critical Rules section first** — these are non-negotiable design constraints
3. Read `../../LEARNINGS.md` (past discoveries and mistakes)
4. Work through tasks in order, updating checkbox status as you complete them
5. **ALWAYS UPDATE ../../LEARNINGS.md** after each completed task/milestone

---

**Critical Rules Summary:**

1. `has_events()` caches events, `poll()` drains them — no duplicate work
2. `has_events()` returns `false` when no events — avoids busy-looping
3. Use `TaskStatus::Depends(shared)` not `TaskStatus::Delayed(timeout)`
4. Use `collect_one()` for finite collection, never `collect_result()` on infinite watchers
5. `StopSignal` for clean task termination
6. Watch BEFORE creating files in tests
7. No `unsafe` for const-to-mut casting

---

_Created: 2026-06-03_
