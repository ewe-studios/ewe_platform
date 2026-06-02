# Feature Start File

## Feature: 00-core-architecture-analysis

**Purpose:** Deep dive analysis of valtron executor architecture

## Agent Workflow (13 Steps)

### Phase 1: Preparation

**Step 1: Read Feature Requirements**
- Read `feature.md` (this feature's requirements)
- Understand the analysis scope and deliverables

**Step 2: Read Supporting Documentation**
- Read parent spec `requirements.md` for context
- Read `AGENTS.md` for agent rules
- Read relevant skills from `.agents/skills/`

### Phase 2: Code Analysis

**Step 3: Read Core Files**
Analyze these files in order:
1. `backends/foundation_core/src/valtron/executors/local.rs` - LocalThreadExecutor
2. `backends/foundation_core/src/valtron/executors/threads.rs` - ThreadYielder, PoolGuard
3. `backends/foundation_core/src/valtron/executors/single/mod.rs` - Single-threaded
4. `backends/foundation_core/src/valtron/executors/multi/mod.rs` - Multi-threaded
5. `backends/foundation_core/src/valtron/executors/drivers.rs` - Stream drivers
6. `backends/foundation_core/src/synca/event.rs` - LockSignal

**Step 4: Generate Analysis**
- Document component behaviors
- Map state machines
- Identify footguns
- Catalog differences

### Phase 3: Documentation

**Step 5: Create compacts if needed**
If approaching context limits:
- Create `compacted.md` with findings
- Clear context, reload from file
- Continue analysis

**Step 6: Complete Analysis**
- Fill in all sections of feature.md
- Update task checklist
- Mark tasks complete as analysis progresses

### Phase 4: Reporting

**Step 7: Update LEARNINGS.md**
Document key findings in parent spec's LEARNINGS.md

**Step 8: Report to User**
Present analysis findings for user review

## Critical Analysis Points

### Must Document
1. TaskStatus variant behaviors
2. State machine transitions
3. CondVar/park_timeout mismatch
4. Single vs multi differences
5. WASM compatibility gaps
6. Shutdown sequence issues

### Must Identify
1. All footguns with triggers
2. Performance bottlenecks
3. Thread safety violations
4. Edge cases

## Files to Analyze

| File | Purpose | Lines |
|------|---------|-------|
| local.rs | Core executor | ~1100 |
| threads.rs | ThreadYielder, PoolGuard | ~500 |
| single/mod.rs | WASM/single-threaded | ~400 |
| multi/mod.rs | Multi-threaded pool | ~400 |
| drivers.rs | Stream polling | ~300 |
| event.rs | LockSignal | ~100 |

## Success Criteria

- [ ] All 12 tasks in feature.md completed
- [ ] Component behaviors documented
- [ ] State machines visualized
- [ ] Footguns catalogued
- [ ] User review completed

---

*Version: 1.0 | Created: 2026-05-11*
