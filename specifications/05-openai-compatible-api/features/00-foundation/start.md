---
purpose: "Agent workflow for implementing the Foundation feature"
version: "1.0"
created: 2026-03-08
---

# Foundation Feature Workflow

## Overview

This feature creates the foundational layer for the OpenAI-compatible API: error types, HTTP client integration, and base types.

## Agent Workflow

### Phase 1: Context Loading

1. **Read feature requirements**
   - Read `feature.md` in this directory

2. **Read related documentation**
   - `../requirements.md` - Full specification context
   - `../../02-build-http-client/requirements.md` - HTTP client patterns
   - `.agents/stacks/rust.md` - Rust conventions

3. **Read existing code**
   - `backends/foundation_core/src/wire/simple_http/client/client.rs` - HTTP client API
   - `backends/foundation_ai/src/lib.rs` - Current structure
   - `backends/foundation_ai/src/errors/mod.rs` - Existing error patterns

4. **Review skills**
   - `.agents/skills/specifications-management/skill.md`
   - `.agents/skills/context-work-ethic/skill.md`

### Phase 2: Implementation

5. **Generate compacted.md** with current state before starting work

6. **Clear context and reload** from saved files

7. **Work on ONE task at a time** from `feature.md`

8. **TDD approach**: Write tests before implementation when possible

9. **Place tests in correct location** - follow language testing skill or project test structure

### Phase 3: Verification & Reporting

9. **Report to Main Agent** after completing each task:
   - What was done
   - Files modified
   - Any issues encountered

10. **Wait for verification** before committing

11. **Update LEARNINGS.md** after each milestone

12. **Delete compacted.md** after commit

## Tasks Summary

See `feature.md` for complete task list. Key tasks:

1. Create error types module
2. Create base types module
3. Create HTTP client wrapper
4. Create common parameters type
5. Add serde dependencies
6. Create module structure
7. Write unit tests
8. Update documentation

## Next Action

Start with Task 1: Create error types module in `backends/foundation_ai/src/openai/errors.rs`
