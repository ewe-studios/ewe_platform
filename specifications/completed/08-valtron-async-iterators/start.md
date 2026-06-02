---
workspace_name: "ewe_platform"
spec_directory: "specifications/08-valtron-async-iterators"
this_file: "specifications/08-valtron-async-iterators/start.md"
created: 2026-03-20
---

# Start: Valtron Async Iterators Specification

## Agent Workflow

This specification uses **feature-based structure** (`has_features: true`). Follow this workflow:

### Step 1: Read Specification Overview
1. Read `requirements.md` - High-level overview, goals, and Feature Index
2. Read `../LEARNINGS.md` - Past discoveries and design decisions

### Step 2: Identify Your Feature
Consult the **Feature Index** in `requirements.md` to find which feature you're implementing:

| # | Feature | File |
|---|---------|------|
| 1 | foundation | `features/00-foundation/start.md` |
| 2 | task-iterators | `features/01-task-iterators/start.md` |
| 3 | stream-iterators | `features/02-stream-iterators/start.md` |
| 4 | collection-combinators | `features/03-collection-combinators/start.md` |
| 5 | mapping-combinators | `features/04-mapping-combinators/start.md` |
| 6 | unified-executor-integration | `features/05-unified-executor-integration/start.md` |
| 6a | client-request-refactor | `features/06a-client-request-refactor/start.md` |
| 6b | map-iter-combinator | `features/06b-map-iter-combinator/start.md` |
| 6c | gen-model-descriptors-parallel-fetch | `features/06c-gen-model-descriptors-parallel-fetch/start.md` |
| 7 | split-collector | `features/07-split-collector/start.md` |

### Step 3: Navigate to Feature Workflow
Navigate to the corresponding `features/[feature-name]/start.md` file and follow that feature's complete workflow.

### Step 4: After Feature Completion
1. Update `../LEARNINGS.md` with discoveries from that feature
2. Mark task as complete in `requirements.md` frontmatter
3. Proceed to next feature (respecting dependencies)

---

**Workflow:** Requirements → Learnings → Select Feature → Feature start.md → Implement → Update Learnings → Next Feature

---

_Created: 2026-03-20_
