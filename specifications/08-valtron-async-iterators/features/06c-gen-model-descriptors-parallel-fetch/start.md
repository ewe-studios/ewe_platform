---
workspace_name: "ewe_platform"
spec_directory: "specifications/08-valtron-async-iterators"
this_file: "specifications/08-valtron-async-iterators/features/06c-gen-model-descriptors-parallel-fetch/start.md"
created: 2026-03-23
---

# Start: gen_model_descriptors Parallel Fetch Feature

## Feature Overview

**Goal**: Use `execute_collect_all()` to fetch model metadata from multiple APIs in parallel.

**Problem**: Current implementation fetches from 3 APIs sequentially (~1500ms total).

**Solution**: Use `execute_collect_all()` to run all fetches in parallel (~500ms total).

---

## Workflow

### Step 1: Read Prerequisites
1. Read `../05-unified-executor-integration/feature.md` - Understand `execute_collect_all()` pattern
2. Read `../01-task-iterators/feature.md` - TaskIterator combinators for building fetch tasks

### Step 2: Implement Parallel Fetch

1. **Create `FetchPending` enum** - Progress states with source tracking
2. **Create `create_fetch_task()` helper** - Compose SendRequestTask with combinators
3. **Create parser functions** - Transform HttpResponse → Vec<ModelEntry> for each API
4. **Update `run()` function** - Use `execute_collect_all()` with composed tasks
5. **Add benchmark timing** - Log fetch elapsed time

### Step 3: Test

```bash
cargo run --bin ewe_platform gen_model_descriptors
# Verify output matches original
# Verify ~3x speedup (1500ms → 500ms)
```

---

## Files to Modify

- `backends/ewe_platform/src/bin/gen_model_descriptors.rs` - Main implementation

---

## Verification

```bash
cargo check -p ewe_platform
cargo run --bin ewe_platform gen_model_descriptors
cargo clippy -p ewe_platform -- -D warnings
cargo fmt -p ewe_platform -- --check
```

---

**Dependencies**: This feature depends on `execute_collect_all()` from feature 05, which is already complete.

---

_Created: 2026-03-23_
