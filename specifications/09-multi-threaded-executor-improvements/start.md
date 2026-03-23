---
workspace_name: "ewe_platform"
spec_directory: "specifications/09-multi-threaded-executor-improvements"
this_file: "specifications/09-multi-threaded-executor-improvements/start.md"
created: 2026-03-23
---

# Start: Multi-Threaded Executor Improvements

## Agent Workflow

1. Read `requirements.md`
2. Read `LEARNINGS.md` (if exists)
3. Read `PROGRESS.md` (if exists)
4. Read `.agents/AGENTS.md` to identify your agent type
5. Read your agent file in `.agents/agents/[agent-name].md`
6. Read skills specified in your agent documentation
7. **MANDATORY**: Generate `compacted.md` with all info using `.agents/skills/context-compaction/skill.md`
8. Clear context, reload from `compacted.md` only, start work
9. **Work on ONE item at a time** - one task at a time - finish it completely before next
10. Implement following TDD (test first, then code) - **one test at a time**
11. Report to Main Agent when done (DO NOT commit)
12. Wait for verification to pass
13. After commit: delete `compacted.md`, update `PROGRESS.md`, move to next task

---

**Workflow:** Requirements → Learnings → Progress → AGENTS.md → Agent Doc → Skills → **Compact → Clear → Reload** → **ONE ITEM AT A TIME** → Implement → Report → Verify → Commit → Delete compacted.md → Next

---

## Target Module

**File**: `backends/foundation_core/src/valtron/executors/multi/mod.rs`

## Issue Summary

This specification addresses 6 issues in the multi-threaded executor:

1. **Global State Contamination Risk** (Lines 16-17) - `OnceLock` statics cannot be reset between tests
2. **Panic-Based Error Handling** (Lines 23-24, 97-99, 120-122) - Public APIs panic instead of returning Result
3. **Inefficient Lock Usage** (Lines 161-171) - `DCounter` acquires same mutex 3 times
4. **Signal Handler Conflicts** (Lines 57-68) - Panics if another handler is already set
5. **No Timeout Protection** (Line 40) - `block_on` can block indefinitely
6. **Lock Contention in Counter** (Lines 195-205) - Holds mutex during channel send

## Task Priority Order

1. Optimize `DCounter` lock usage (simplest)
2. Optimize `Counter` lock usage (related)
3. Replace panics with Result types
4. Add `block_on_timeout` variant
5. Make signal handler graceful
6. Add test reset mechanism (most complex)

---

_Created: 2026-03-23_
