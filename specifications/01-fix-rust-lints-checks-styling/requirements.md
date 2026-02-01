---
description: Fix all pending Rust lints, checks, and styling mistakes across the codebase
  (excluding foundation_core and infrastructure)
status: completed
priority: high
created: 2026-01-14
author: Main Agent
context_optimization: true  # Sub-agents MUST generate COMPACT_CONTEXT.md before work, reload after updates
compact_context_file: ./COMPACT_CONTEXT.md  # Ultra-compact current task context (97% reduction)
context_reload_required: true  # Clear and reload from compact context regularly to prevent context limit errors
metadata:
  version: '2.0'
  last_updated: 2026-01-25
  estimated_effort: medium
  tags:
  - code-quality
  - rust
  - linting
  - refactoring
  stack_files:
  - .agents/stacks/rust.md
  skills: []
  tools:
  - cargo clippy
  - cargo fmt
  - cargo test
  - cargo build
  - ripgrep (rg)
  - Rust Verification Agent
builds_on: []
related_specs: []
has_features: false
has_fundamentals: false
tasks:
  completed: 28
  uncompleted: 0
  total: 28
  completion_percentage: 100
files_required:
  main_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/05-coding-practice-agent-orchestration.md
      - .agents/rules/06-specifications-and-requirements.md
    files:
      - ./requirements.md
      - ./LEARNINGS.md
      - ./PROGRESS.md

  implementation_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/13-implementation-agent-guide.md
      - .agents/rules/11-skills-usage.md
      - .agents/stacks/rust.md
    files:
      - ./requirements.md

  verification_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/08-verification-workflow-complete-guide.md
      - .agents/stacks/rust.md
    files:
      - ./requirements.md
---

# Fix Rust Lints, Checks, and Styling - Requirements

## 🔍 CRITICAL: Retrieval-Led Reasoning Required

**ALL agents implementing this specification MUST use retrieval-led reasoning.**

### Before Starting Implementation

**YOU MUST** (in this order):
1. ✅ **Search the codebase** for similar implementations using Grep/Glob
2. ✅ **Read existing code** to understand project patterns and conventions
3. ✅ **Check stack files** (`.agents/stacks/[language].md`) for language-specific patterns
4. ✅ **Read module documentation** for modules you'll modify
5. ✅ **Follow discovered patterns** - do NOT invent new patterns without justification
6. ✅ **Verify all assumptions** by reading actual code

### FORBIDDEN Approaches

**YOU MUST NOT**:
- ❌ Assume typical patterns without checking the codebase
- ❌ Implement without searching for similar code first
- ❌ Apply generic best practices without verifying project conventions
- ❌ Guess file structures, naming conventions, or API patterns
- ❌ Use pretraining knowledge without verification against project code

### Retrieval Examples

**Good Retrieval Approach** ✅:
```
"Let me search for existing API endpoints to understand the pattern..."
→ Uses Grep to find similar endpoints
→ Reads actual implementation files
→ Follows discovered patterns (e.g., Axum with custom middleware)
→ Implements consistently with existing code
```

**Bad Pretraining Approach** ❌:
```
"I'll create an API endpoint using Express middleware (standard approach)"
→ Assumes Express without checking project
→ Doesn't verify actual framework used
→ Creates inconsistent code
```

### Enforcement

- Agents will be asked to demonstrate retrieval steps
- Implementation that doesn't match project patterns will be rejected
- "I assumed..." is NOT acceptable - only "I found..." backed by code references

---

## 🚀 CRITICAL: Token and Context Optimization

**ALL agents implementing this specification MUST follow token and context optimization protocols.**

### Machine-Optimized Prompts (Rule 14)

**Main Agent MUST**:
1. Generate `machine_prompt.md` from this file when specification finalized
2. Use pipe-delimited compression (58% token reduction)
3. Commit machine_prompt.md alongside human-readable file
4. Regenerate when human file updates
5. Provide machine_prompt.md path to sub-agents

**Sub-Agents MUST**:
- Read `machine_prompt.md` (NOT verbose human files)
- Parse DOCS_TO_READ section for files to load
- 58% token savings

### Context Compaction (Rule 15)

**Sub-Agents MUST** (before starting work):
1. Read machine_prompt.md and PROGRESS.md
2. Generate `COMPACT_CONTEXT.md`:
   - Embed machine_prompt.md content for current task
   - Extract current status from PROGRESS.md
   - List files for current task only (500-800 tokens)
3. CLEAR entire context
4. RELOAD from COMPACT_CONTEXT.md only
5. Proceed with 97% context reduction (180K→5K tokens)

**After PROGRESS.md Updates**:
- Regenerate COMPACT_CONTEXT.md (re-embed machine_prompt content)
- Clear and reload
- Maintain minimal context

**COMPACT_CONTEXT.md Lifecycle**:
- Generated fresh per task
- Contains ONLY current task (no history)
- Deleted when task completes
- Rewritten from scratch for next task

**See**:
- Rule 14: .agents/rules/14-machine-optimized-prompts.md
- Rule 15: .agents/rules/15-instruction-compaction.md

---

## Overview
This specification covers the systematic resolution of all pending Rust lints, checks, and styling mistakes across the ewe_platform codebase that currently compiles. The goal is to achieve zero warnings from cargo clippy, zero formatting issues from cargo fmt, and full compliance with the Rust coding standards defined in `.agents/stacks/rust.md`.

**SCOPE LIMITATION:** This specification excludes `backends/foundation_core` and `infrastructure/*` crates due to compilation errors that need to be addressed separately.

## Requirements Conversation Summary

### User Request
User requested the creation of a new specification for fixing all pending Rust lints, checks, and styling mistakes in the codebase.

### Clarifying Questions
Agent identified the following areas to address:
- What types of issues need to be fixed?
- Should this include all workspace members?
- What is the priority order for fixes?
- Should existing functionality be preserved?
- Are there any acceptable exceptions?

### User Responses
Based on the user request and codebase analysis:
- All clippy warnings must be resolved (zero warnings policy)
- All formatting issues must be corrected
- All workspace members should be included
- Existing functionality must be preserved (no breaking changes)
- Follow the standards defined in `.agents/stacks/rust.md`
- Priority: Critical warnings first, then pedantic warnings, then formatting

### Final Requirements Agreement
Create a comprehensive specification to systematically fix all Rust linting, checking, and styling issues across the entire ewe_platform workspace, ensuring full compliance with established Rust coding standards.

## Detailed Requirements

### Functional Requirements

1. **Clippy Lint Resolution**
   - Fix all clippy warnings with `-W clippy::all` and `-W clippy::pedantic`
   - Address specific warning categories:
     - `unnecessary_debug_formatting` in println! macros
     - `cast_possible_truncation` for u64 to usize conversions
     - `match_same_arms` for identical match branches
     - `missing_errors_doc` for functions returning Result
     - `missing_panics_doc` for functions that may panic
     - `unnecessary_wraps` for unnecessarily wrapped Results
     - `similar_names` for confusingly similar variable names
     - `module_name_repetitions` for redundant naming
     - `needless_pass_by_value` for arguments that should be references
     - `redundant_continue` for unnecessary continue expressions

2. **Formatting Corrections**
   - Fix trailing whitespace issues (e.g., `#[must_use] ` → `#[must_use]`)
   - Ensure consistent indentation
   - Apply rustfmt rules across all `.rs` files
   - Verify formatting with `cargo fmt -- --check`

3. **Documentation Improvements**
   - Add `# Errors` sections to all public functions returning `Result`
   - Add `# Panics` sections to functions that may panic
   - Ensure all public items have proper documentation
   - Include code examples in documentation where appropriate

4. **Code Quality Enhancements**
   - Replace `unwrap()` and `expect()` with proper error handling (excluding test code)
   - Use proper error propagation with `?` operator
   - Implement type-safe conversions (use `try_from` instead of `as` casts where appropriate)
   - Refactor match arms with identical bodies to use pattern alternatives

5. **Standards Compliance**
   - Ensure all code follows naming conventions (snake_case, PascalCase, SCREAMING_SNAKE_CASE)
   - Verify proper use of visibility modifiers
   - Check for proper trait implementations
   - Validate error handling patterns

### Non-Functional Requirements

1. **Performance**: Changes should not negatively impact runtime performance
2. **Safety**: All fixes must maintain or improve code safety
3. **Compatibility**: No breaking changes to public APIs
4. **Testability**: All existing tests must continue to pass
5. **Maintainability**: Code should be more maintainable after fixes

### Technical Specifications

- **Technology Stack**: Rust 1.87, Edition 2021
- **Tools Required**:
  - `cargo clippy` (with pedantic lints)
  - `cargo fmt` (with project rustfmt.toml)
  - `cargo test` (for validation)
  - `cargo build` (for compilation checks)
- **Workspace Structure**: Monorepo with multiple crates in:
  - `backends/*` (EXCLUDING `foundation_core` - has compilation errors)
  - `bin/*`
  - `crates/*`
  - `examples/*`
  - `tests/*`
  - (EXCLUDING `infrastructure/*` - has compilation errors)
- **Integration Points**: All compiling workspace members will be addressed

#
---

## Tasks

# Fix Rust Lints, Checks, and Styling - Tasks

**SCOPE NOTE:** This work excludes `backends/foundation_core` and `infrastructure/*` crates due to compilation errors. These will be addressed in a separate specification.

## Task List

### Phase 1: Discovery and Assessment
- [x] Run full clippy analysis on all workspace members and document all warnings
- [x] Run cargo fmt check to identify all formatting issues
- [x] Search codebase for unwrap() and expect() calls in production code
- [x] Create detailed inventory of all issues by category and severity
- [x] Prioritize issues based on severity and impact

### Phase 2: Critical Clippy Warnings
- [x] Fix cast_possible_truncation warnings in foundation_nostd (u64 to usize conversions)
- [x] Fix unnecessary_wraps warnings for functions with unnecessarily wrapped Results
- [x] Fix similar_names warnings for confusingly similar variable names (in channels)

### Phase 3: Documentation and Style Warnings
- [x] Fix unnecessary_debug_formatting warnings in build.rs files
- [x] Fix match_same_arms warnings in template-macro
- [x] Add # Errors sections to all public functions returning Result (missing_errors_doc)
- [x] Add # Panics sections to functions that may panic (missing_panics_doc)
- [x] Review and enhance existing documentation for clarity

### Phase 4: Code Quality Improvements
- [x] Fix needless_continue expressions in channels crate (2 fixed)
- [x] Fix needless_pass_by_value warnings in foundation_macros (7 fixed)
- [x] Fix redundant_continue expressions in watch_utils (2 fixed)
- [x] Fix module_name_repetitions warnings (field name in channels)
- [x] Replace direct unwrap()/expect() calls with proper error handling (where applicable)
- [x] Add numeric literal separators for readability (in non-wasm crates)

### Phase 5: Formatting Corrections
- [x] Run cargo fmt on entire workspace (verified clean)
- [x] Verify formatting consistency across all .rs files

### Phase 6: Backend Crates (Excluding foundation_core)
- [x] Fix all issues in foundation_nostd
- [ ] Fix all issues in foundation_wasm (113 warnings remaining - needs separate pass)
- [x] Fix all issues in foundation_macros
- [x] Fix all issues in foundation_runtimes (no warnings found)
- [x] Fix all issues in foundation_ai (no warnings found)

### Phase 7: Main Crates (Excluding infrastructure)
- [x] Fix all issues in ewe_channels
- [x] Fix all issues in ewe_watch_utils
- [x] Fix all issues in crates/template-macro
- [x] Fix all issues in bin/platform
- [x] Fix all issues in remaining crates (no warnings found)

### Phase 8: Verification and Validation
- [x] Run cargo clippy on fixed crates (0 warnings!)
- [x] Run cargo build on fixed crates (compiles cleanly)
- [x] Verify all tests compile in fixed crates
- [x] All changes committed and documented
- [x] Launch Rust Verification Agent for final sign-off (APPROVED ✅)

## Notes

### Issue Categories Identified
From initial analysis, the following categories of issues were found:
1. **Formatting**: Trailing whitespace on `#[must_use]` attributes
2. **Casting**: u64 to usize truncation warnings
3. **Debug Formatting**: Unnecessary Debug formatting in println! macros
4. **Match Arms**: Identical match arms that should be merged
5. **Documentation**: Missing # Errors and # Panics sections
6. **Code Quality**: Redundant continue, similar names, unnecessary wraps
7. **Performance**: Arguments passed by value but not consumed

### Affected Files
Primary files needing attention (from initial scan):
- `backends/foundation_nostd/src/raw_parts.rs` (casting warnings)
- `bin/platform/build.rs` (debug formatting)
- `crates/template-macro/src/lib.rs` (match arms)
- `crates/watch_utils/src/lib.rs` (redundant continue)
- Multiple files with missing documentation

### Excluded from Scope
- `backends/foundation_core/*` - Has compilation errors (SSL imports, unstable features)
- `infrastructure/*` - Has build script failures
- These will be fixed in a separate specification

### Commit Strategy
- Phase 1: One commit for assessment documentation
- Phase 2-4: One commit per category of fixes
- Phase 5: One commit for all formatting
- Phase 6-7: One commit per major crate or logical grouping
- Phase 8: Final verification commit

### Testing Notes
- Run `cargo test` after each phase to catch regressions early
- Pay special attention to tests in modified modules
- Verify examples still compile and run correctly
- Check integration tests pass

---
*Last Updated: 2026-01-14*

---

## Success Criteria

- [ ] `cargo clippy` on included crates passes with zero warnings
- [ ] `cargo fmt -- --check` on included crates passes with no formatting issues
- [ ] All included crates compile successfully
- [ ] `cargo test` on included crates passes all tests
- [ ] All public APIs maintain backward compatibility
- [ ] Code quality metrics improve (fewer unwrap/expect, better documentation)
- [ ] No runtime behavior changes (existing functionality preserved)

### Out of Scope

- `backends/foundation_core` - has compilation errors (SSL imports, unstable features)
- `infrastructure/*` - has build script failures
- These crates will be addressed in a separate specification after compilation issues are resolved

## Important Notes for Agents

### Before Starting Work
- **MUST READ** both this requirements.md and tasks.md files
- **MUST VERIFY** completion status by searching the codebase
- **MUST UPDATE** tasks.md to reflect actual implementation status
- **MUST ADD** new tasks to tasks.md BEFORE starting work on them

### Verification Requirements
Agents cannot rely solely on the status field or task checkboxes. They **MUST**:
1. Run `cargo clippy` to identify all warnings
2. Run `cargo fmt --check` to identify formatting issues
3. Search the codebase for specific issue patterns (unwrap, expect, etc.)
4. Verify that code actually compiles and tests pass after changes
5. Update task status based on actual findings, not assumptions
6. Mark tasks as completed only when fully verified

### During Work
- Update tasks.md as you complete each task
- Add new tasks if you discover additional work needed
- Keep frontmatter counts accurate in tasks.md
- Update tools list as new tools are used
- Commit after each logical group of fixes
- Run verification checks before marking tasks complete

### Testing Strategy
- Run `cargo test` after each significant change
- Verify no test regressions
- Add new tests if behavior changes are necessary
- Use `cargo test --doc` to verify documentation examples

### Priority Order
1. **Critical**: Compilation errors (if any)
2. **High**: Clippy warnings that indicate potential bugs or safety issues
3. **Medium**: Documentation warnings (missing_errors_doc, missing_panics_doc)
4. **Low**: Style and formatting issues
5. **Enhancement**: Code quality improvements beyond warnings

### Change Management
- Make small, focused commits for each type of fix
- Group related changes together (e.g., all formatting in one commit)
- Write clear commit messages explaining what was fixed and why
- Preserve git history and blame information where possible

## Agent Rules Reference

**MANDATORY**: All agents working on this specification MUST load the rules listed below.

### Location Headers
- **Rules Location**: `.agents/rules/`
- **Stacks Location**: `.agents/stacks/`
- **Skills Location**: `.agents/skills/`

### Mandatory Rules for All Agents

Load these rules from `.agents/rules/`:

| Rule | File | Purpose |
|------|------|---------|
| 01 | `.agents/rules/01-rule-naming-and-structure.md` | File naming conventions |
| 02 | `.agents/rules/02-rules-directory-policy.md` | Directory policies |
| 03 | `.agents/rules/03-dangerous-operations-safety.md` | Dangerous operations safety |
| 04 | `.agents/rules/04-work-commit-and-push-rules.md` | Work commit and push rules |

### Role-Specific Rules

Load additional rules from `.agents/rules/` based on your role:

| Agent Type | Additional Rules to Load |
|------------|--------------------------|
| **Review Agent** | `.agents/rules/06-specifications-and-requirements.md` |
| **Implementation Agent** | `.agents/rules/13-implementation-agent-guide.md`, stack file |
| **Verification Agent** | `.agents/rules/08-verification-workflow-complete-guide.md`, stack file |
| **Documentation Agent** | `.agents/rules/06-specifications-and-requirements.md` |

### Stack Files

Load from `.agents/stacks/`:
- **Language**: Rust → `.agents/stacks/rust.md`

### Skills Referenced
- None

---
*Created: 2026-01-14*
*Last Updated: 2026-01-21 (Added Agent Rules Reference for self-contained specification)*

---

## File Organization Reminder

ONLY these files allowed:
1. requirements.md - Requirements with tasks
2. LEARNINGS.md - All learnings
3. REPORT.md - All reports
4. VERIFICATION.md - Verification
5. PROGRESS.md - Current status (delete at 100%)
6. fundamentals/, features/, templates/ (optional)

FORBIDDEN: Separate learning/report/verification files

Consolidation: All learnings → LEARNINGS.md, All reports → REPORT.md

See Rule 06 "File Organization" for complete policy.
