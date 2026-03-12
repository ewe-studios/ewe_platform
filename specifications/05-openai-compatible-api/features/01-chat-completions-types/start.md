---
purpose: "Agent workflow for implementing Chat Completions Types feature"
version: "1.0"
created: 2026-03-08
---

# Chat Completions Types Feature Workflow

## Overview

Define all types required for the OpenAI Chat Completions API.

## Agent Workflow

### Phase 1: Context Loading

1. **Read feature requirements**
   - Read `feature.md` in this directory

2. **Read related documentation**
   - `../requirements.md` - Full specification context
   - `../00-foundation/feature.md` - Foundation types

3. **Read existing code**
   - `backends/foundation_ai/src/openai/mod.rs` - Module structure
   - `backends/foundation_ai/src/openai/types.rs` - Base types

4. **Review skills**
   - `.agents/skills/specifications-management/skill.md`
   - `.agents/skills/context-work-ethic/skill.md`

### Phase 2: Implementation

5. **Generate compacted.md** with current state

6. **Clear context and reload**

7. **Work on ONE task at a time**

8. **TDD approach**: Write serialization tests first

9. **Place tests in correct location** - follow language testing skill or project test structure

### Phase 3: Verification & Reporting

9. **Report to Main Agent** after each task

10. **Wait for verification**

11. **Update LEARNINGS.md**

12. **Delete compacted.md** after commit

## Tasks Summary

1. Create Chat Completions types module
2. Create streaming event types
3. Implement serialization helpers
4. Create module structure
5. Write unit tests
6. Add documentation

## Next Action

Start with Task 1: Create `backends/foundation_ai/src/openai/chat/types.rs`
