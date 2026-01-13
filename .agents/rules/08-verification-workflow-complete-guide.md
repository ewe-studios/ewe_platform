# Verification Workflow - Complete Integration Guide

## Purpose
This rule provides a comprehensive guide to the iron-clad verification workflow system that ensures **NO code is EVER committed without passing ALL quality checks**. This is an **IRON-CLAD REQUIREMENT** with **ZERO TOLERANCE** for violations.

This guide integrates and summarizes the complete verification workflow from Rules 03, 04, 05, 07 and all stack files, providing a single reference for understanding the entire system.

## Rule

### Overview

This document summarizes the comprehensive mandatory code verification workflow system that ensures **NO code is EVER committed without passing ALL quality checks**. This is an **IRON-CLAD REQUIREMENT** with **ZERO TOLERANCE** for violations.

---

## System Architecture

### Core Principle

```
Implementation → Report → Verify → Pass? → Commit → Push
                            ↓
                          Fail → Urgent Task → Fix → Loop
```

**CRITICAL RULES**:
- Code commits happen ONLY after verification passes
- Implementation agents **NEVER** commit directly
- **ONLY Main Agent can spawn verification agents**

### Agent Hierarchy and Verification Authority

**Main Agent** (Top of hierarchy):
- ✅ Directly interacting with user
- ✅ Orchestrator of all workflows
- ✅ **ONLY agent with authority to spawn verification agents**
- ✅ Spawns: Implementation agents, Specification agents, Verification agents

**Sub-Agents** (Implementation, Specification, etc.):
- ❌ **NEVER spawn verification agents**
- ❌ Do NOT have verification authority
- ✅ Report completion to Main Agent
- ✅ Wait for Main Agent to orchestrate verification

**Identity Rule**:
```
If you were spawned by another agent → You are a SUB-AGENT
If you are directly interacting with user → You are the MAIN AGENT

SUB-AGENTS: NEVER spawn verification agents, report to Main Agent
MAIN AGENT: Spawn verification agents, orchestrate verification
```

---

## The Four-Phase Workflow

### Phase 1: Implementation

**What Happens:**
- Implementation agent reads AGENTS.md, all rules, relevant stack files, and specifications
- **Implementation agent recognizes it is a SUB-AGENT** (spawned by Main Agent)
- Implements code following all documented standards
- Writes tests for new functionality
- Keeps track of changes made

**CRITICAL**: After completing work, implementation agent:
1. ✅ **REPORTS to Main Agent** (provides changed files, description, language(s) used)
2. ✅ **STOPS and WAITS** for Main Agent response
3. ❌ **DOES NOT commit** anything
4. ❌ **DOES NOT push** anything
5. ❌ **DOES NOT update** tasks.md directly
6. ❌ **DOES NOT spawn verification agents** (only Main Agent has this authority)

### Phase 2: Mandatory Verification

**What Happens:**
- Main Agent analyzes changed files to identify language(s) modified
- Main Agent spawns **ONE verification agent per language stack** (NEVER more)
- Verification agent receives context (files, description, specification)
- Verification agent runs **ALL checks** defined in stack file:

**For Rust** (from `.agents/stacks/rust.md`):
1. `cargo fmt -- --check` (format)
2. `cargo clippy -- -D warnings` (lint, zero warnings)
3. `cargo test` or `cargo test --package [crate]` (tests)
4. `cargo build --all-features` (build)
5. `cargo doc --no-deps` (documentation)
6. `cargo audit` (security)
7. Standards compliance checks (no unwrap(), proper docs, etc.)

**For JavaScript/TypeScript** (from `.agents/stacks/javascript.md`):
1. `npx prettier --check .` (format)
2. `npx tsc --noEmit` (type check, zero errors)
3. `npx eslint . --max-warnings 0` (lint, zero warnings)
4. `npm test` (tests with coverage)
5. `npm run build` (build)
6. `npm audit` (security)
7. Standards compliance checks (no `any` type, etc.)

**For Python** (from `.agents/stacks/python.md`):
1. `black --check .` (format)
2. `ruff check .` (lint, zero errors)
3. `mypy .` (type check, strict mode)
4. `pytest --cov` (tests with coverage)
5. `python -m py_compile src/**/*.py` (import check)
6. `pip-audit` or `bandit` (security)
7. Standards compliance checks (no mutable defaults, etc.)

**Verification Agent Generates Report:**
```markdown
# [Language] Verification Report

## Status: PASS ✅ / FAIL ❌

## Files Verified
- [list of files]

## Check Results
1. Format: PASS ✅ / FAIL ❌
2. Lint: PASS ✅ / FAIL ❌
3. Type Check: PASS ✅ / FAIL ❌
4. Tests: PASS ✅ / FAIL ❌ ([N] passed, [N] failed)
5. Build: PASS ✅ / FAIL ❌
6. Security: PASS ✅ / FAIL ❌
7. Standards: PASS ✅ / FAIL ❌

## Test Results
- Total: [N]
- Passed: [N]
- Failed: [N]
- Coverage: [N]%

## Details
[Specific errors if any]
```

### Phase 3: Main Agent Decision

**IF ALL Verifications PASS ✅:**

1. Main Agent identifies related specification (if applicable)
2. Main Agent spawns **Specification Update Agent** (NEVER updates directly)
3. Main Agent provides Specification Agent with context:
   - Verification report (PASS status)
   - Completed tasks list
   - Files changed
   - Implementation description
4. Specification Agent reads `.agents/specifications/NN-spec-name/tasks.md`
5. Specification Agent marks relevant tasks as `[x]` completed
6. Specification Agent updates frontmatter (`completed: N`, `uncompleted: M`)
7. Specification Agent deletes `verification.md` if exists (cleanup from previous failure)
8. Specification Agent saves tasks.md and reports completion to Main Agent
9. Main Agent commits code AND specification updates with **verification status** (Rule 03):
   ```
   Brief summary of change

   Detailed explanation...

   Changes made:
   - Change 1
   - Change 2
   - Change 3

   Verified by [Language] Verification Agent: All checks passed
   - Format: PASS
   - Lint: PASS
   - Type Check: PASS
   - Tests: [N]/[N] PASS
   - Build: PASS
   - Coverage: [N]%

   Specification: .agents/specifications/NN-name/
   Tasks completed: [N]
   Tasks remaining: [M]

   Co-Authored-By: Claude <noreply@anthropic.com>
   ```
10. Main Agent automatically pushes to remote (Rule 05)
11. Main Agent reports success to user

**CRITICAL**: Main Agent MUST NOT read/write tasks.md directly - ALWAYS delegates to Specification Agent.

**IF ANY Verification FAILS ❌:**

1. Main Agent **DOES NOT COMMIT** any code
2. Main Agent **DOES NOT PUSH** anything
3. Main Agent identifies related specification directory
4. Main Agent spawns **Specification Update Agent** (NEVER updates directly)
5. Main Agent provides Specification Agent with context:
   - Full verification FAIL report
   - Failed checks details
   - Files affected
   - Recommended fixes
6. Specification Agent creates **verification.md** in specification directory:
   ```markdown
   # Verification Report - FAILED

   **Status**: FAIL ❌
   **Date**: [timestamp]
   **Language**: [Rust/JavaScript/Python]
   **Specification**: [path]

   ## Failed Checks
   [Detailed errors with line numbers, stack traces, etc.]

   ## Files Affected
   [List of files with specific issues]

   ## Recommended Fixes
   [Step-by-step fix instructions]

   ## Agent Action Required
   Read this report, fix all issues, mark urgent task done,
   report completion to Main Agent, verification will run again.
   ```
7. Specification Agent adds **NEW URGENT TASK at TOP** of tasks.md:
   ```markdown
   ## URGENT: Failed Verification Tasks
   - [ ] **FIX: Verification failures in [feature-name]**
     - Verification failed on [date/time]
     - Language: [Rust/JavaScript/Python]
     - Failed checks: [summary]
     - See verification.md for detailed report
     - Files affected: [list]
     - Must fix before proceeding
   ```
8. Specification Agent updates frontmatter (`uncompleted: N+1`)
9. Specification Agent saves tasks.md and verification.md
10. Specification Agent reports completion to Main Agent
11. Main Agent reports detailed failures to user with fix recommendations

**CRITICAL**: Main Agent MUST NOT create verification.md or update tasks.md directly - ALWAYS delegates to Specification Agent.

### Phase 4: Fix and Retry (If Verification Failed)

**What Happens:**
1. Main Agent spawns Implementation Agent (or resumes existing agent)
2. Main Agent provides context:
   - Specification path
   - **verification.md location** (key file to read)
   - Urgent task to complete
3. Implementation Agent reads **verification.md** to understand:
   - All failed checks with details
   - Error messages and line numbers
   - Recommended fixes
4. Implementation agent fixes code issues:
   - Addresses ALL failures listed in verification.md
   - Ensures tests pass locally
   - Follows all stack standards
5. Implementation agent marks urgent fix task as `[x]` complete in tasks.md
6. Implementation agent **REPORTS completion to Main Agent again**
7. Main Agent launches verification agents again (back to Phase 2)
8. **IF verification PASSES**:
   - Main Agent spawns Specification Update Agent
   - Specification Agent **deletes verification.md** (no longer needed)
   - Specification Agent marks completed tasks in tasks.md
   - Specification Agent reports completion
   - Main Agent commits all changes (code + specification)
9. **IF verification FAILS again**:
   - Main Agent spawns Specification Update Agent
   - Specification Agent **overwrites verification.md** with new report
   - Specification Agent updates or adds urgent task
   - Process repeats from step 1

**CRITICAL**: This loop continues indefinitely until all checks pass. There is NO bypass.

**verification.md Lifecycle**:
- **Created** by Specification Update Agent on verification FAIL
- **Read** by Implementation Agent to understand fixes needed
- **Overwritten** by Specification Agent on subsequent failures
- **Deleted** by Specification Update Agent on verification PASS
- **Lives** in specification directory beside tasks.md (`.agents/specifications/NN-name/verification.md`)

---

## Race Condition Prevention

**CRITICAL RULE**: Only **ONE verification agent per language stack** at any time.

### Prevention Strategy

Main Agent tracks active verifications:
```
active_verifications = {
  'rust': null,      # No Rust verification running
  'javascript': null, # No JS verification running
  'python': null      # No Python verification running
}

Before spawning verification agent:
  IF active_verifications[language] is not null:
    WAIT for existing agent to complete
  ELSE:
    Spawn new verification agent
    Mark active_verifications[language] = agent_id

After verification completes:
  Mark active_verifications[language] = null
```

### Why This Matters

**Good ✅:**
```
Main Agent sees Rust changes
→ Spawns ONE Rust Verification Agent
→ Waits for completion
→ Agent finishes and reports
→ Proceeds based on results
```

**Bad ❌:**
```
Main Agent sees Rust changes
→ Spawns Rust Verification Agent #1
→ Also spawns Rust Verification Agent #2  ❌ VIOLATION
→ Race condition possible
→ File conflicts
→ Inconsistent results
```

---

## Rule Integration

### Rule 03 (Work Commit Rules)
- Verification happens **BEFORE** commit
- Commit message **MUST include** verification status
- Only verified code is committed
- Specification updates included in commit

### Rule 04 (Agent Orchestration)
- Defines the 4-phase workflow
- Establishes verification as mandatory gate
- ONE verification agent per stack
- Race condition prevention
- Specification update process

### Rule 05 (Git Auto-Approval and Push)
- Push happens **AFTER** successful verification
- No manual approval needed (verification is the gate)
- Automatic push on verification pass
- No push if verification fails (no commit = no push)

### Rule 06 (Specifications and Requirements)
- Verification agent receives specification context
- Tasks.md updated based on verification results
- Failed verification creates urgent task at top
- Successful verification marks tasks complete

### Rule 07 (Language Conventions)
- Verification enforces all stack standards
- Stack files define exact verification commands
- Learning Logs updated with issues found
- Standards continuously improved

### Stack Files (rust.md, javascript.md, python.md)
- Define exact checks verification agents must run
- Provide verification workflow sections
- Document standards to verify against
- Include report format templates
- Show good/bad workflow examples

---

## Zero Tolerance Enforcement

### Critical Violations

The following are **CRITICAL VIOLATIONS** with **ZERO TOLERANCE**:

1. ❌ **Implementation agent commits directly** (bypasses verification)
2. ❌ **Main agent commits without verification** (skips quality gate)
3. ❌ **Verification agent skips checks** (incomplete verification)
4. ❌ **Multiple verification agents for same language** (race condition)
5. ❌ **Committing code with failed verification** (quality breach)
6. ❌ **Not updating specification on verification failure** (lost tracking)

### Consequences

Any violation results in:
1. **IMMEDIATE REVERT** of any committed code
2. **STOP ALL WORK** until violation is corrected
3. **DOCUMENT in Learning Log** (violation details)
4. **REPORT to user** (transparency)
5. **RE-RUN proper workflow** (correct process)

### Why Zero Tolerance

Violations have severe consequences:
- ❌ **Broken builds** in production
- ❌ **Failed tests** discovered too late
- ❌ **Race conditions** from concurrent verifications
- ❌ **Lost work** from reverts
- ❌ **Wasted time** fixing avoidable issues
- ❌ **User frustration** and lost trust

**THE USER WILL BE VERY UPSET** if this workflow is not followed!

---

## Complete Workflow Example (Success)

```
1. User: "Implement user authentication in Rust"

2. Main Agent:
   - Reads specification: .agents/specifications/03-user-authentication/
   - Reads requirements.md and tasks.md
   - Identifies tasks to complete
   - Spawns Rust Implementation Agent

3. Rust Implementation Agent:
   - Reads AGENTS.md
   - Reads all rules from .agents/rules/*
   - Reads .agents/stacks/rust.md
   - Reads specification files
   - Implements authentication code following ALL standards
   - Writes comprehensive tests
   - REPORTS completion to Main Agent:
     * Files: [src/auth/mod.rs, src/auth/token.rs, tests/auth_tests.rs]
     * Language: Rust
     * Description: Implemented JWT-based authentication
     * Specification: 03-user-authentication
   - STOPS and WAITS

4. Main Agent:
   - Receives completion report
   - Identifies language: Rust
   - Checks active_verifications['rust'] = null ✅
   - Spawns ONE Rust Verification Agent
   - Marks active_verifications['rust'] = agent-id-123
   - Provides context (files, spec, description)

5. Rust Verification Agent:
   - Reads .agents/stacks/rust.md
   - Runs cargo fmt -- --check → PASS ✅
   - Runs cargo clippy -- -D warnings → PASS ✅
   - Runs cargo test → PASS ✅ (45 tests passed)
   - Runs cargo build → PASS ✅
   - Runs cargo doc → PASS ✅
   - Runs cargo audit → PASS ✅
   - Checks standards compliance → PASS ✅
   - Generates comprehensive report
   - Reports to Main Agent: PASS ✅

6. Main Agent:
   - Receives PASS report
   - Marks active_verifications['rust'] = null
   - Opens .agents/specifications/03-user-authentication/tasks.md
   - Updates completed tasks:
     * [x] Implement authentication middleware
     * [x] Add JWT token generation
     * [x] Write authentication tests
   - Updates frontmatter: completed: 8, uncompleted: 2
   - Saves tasks.md

7. Main Agent:
   - git add [files]
   - git commit with message:
     ```
     Implement user authentication middleware

     Added JWT token generation and validation with authentication
     middleware for securing API endpoints.

     Changes made:
     - Implemented JWT token generation and validation
     - Created authentication middleware
     - Added comprehensive tests (45 tests, all passing)
     - Updated error handling for auth failures

     Verified by Rust Verification Agent: All checks passed
     - Format: PASS (rustfmt)
     - Lint: PASS (clippy, 0 warnings)
     - Tests: 45/45 PASS
     - Build: PASS (debug and release)
     - Doc: PASS (cargo doc)
     - Security: PASS (cargo audit)
     - Coverage: 89%

     Specification: .agents/specifications/03-user-authentication/
     Tasks completed: 3
     Tasks remaining: 2

     Co-Authored-By: Claude <noreply@anthropic.com>
     ```
   - git status (verify commit)
   - git push (automatic, Rule 05)

8. Main Agent reports success to user:
   ```
   ✅ User authentication implemented successfully!

   Verification Status: PASS
   - All 45 tests passing
   - Code coverage: 89%
   - Zero lint warnings
   - Build successful (debug and release)
   - Documentation complete
   - No security vulnerabilities

   Specification updated: 3 tasks completed, 2 remaining

   Changes pushed to remote: foundation_ai branch
   ```
```

---

## Complete Workflow Example (Failure)

```
1. User: "Add data validation to API endpoints"

2. Main Agent:
   - Reads specification: .agents/specifications/05-data-validation/
   - Spawns Python Implementation Agent

3. Python Implementation Agent:
   - Implements validation logic
   - Writes some tests (but not comprehensive)
   - REPORTS completion to Main Agent

4. Main Agent:
   - Spawns ONE Python Verification Agent

5. Python Verification Agent:
   - Runs black --check → PASS ✅
   - Runs ruff check . → FAIL ❌ (3 lint errors)
   - Runs mypy . → FAIL ❌ (missing type hints)
   - Runs pytest → FAIL ❌ (2 tests failing)
   - Stops checking (failures detected)
   - Generates FAIL report with details

6. Main Agent:
   - Receives FAIL report
   - DOES NOT COMMIT code ❌
   - Opens .agents/specifications/05-data-validation/tasks.md
   - Adds NEW task at TOP:
     ```markdown
     ## URGENT: Failed Verification Tasks
     - [ ] **FIX: Verification failures in data validation**
       - Verification failed on 2026-01-11 15:30
       - Language: Python
       - Failed checks:
         * Lint: 3 errors (unused imports, line too long, undefined var)
         * Type Check: Missing type hints on 5 functions
         * Tests: 2 failing tests
           - test_email_validation: AssertionError
           - test_phone_validation: KeyError
       - Files affected: src/validation.py, tests/test_validation.py
       - Must fix before proceeding
     ```
   - Updates frontmatter: uncompleted: 4 → 5
   - Saves tasks.md
   - DOES NOT push anything ❌

7. Main Agent reports to user:
   ```
   ❌ VERIFICATION FAILED ❌

   Python Verification Agent found issues:

   Lint Errors (3):
   - Line 45: Unused import 're'
   - Line 103: Line too long (115 > 100 characters)
   - Line 67: Undefined variable 'pattern'

   Type Errors (5):
   - Function 'validate_email' missing return type hint
   - Function 'validate_phone' missing parameter type hints
   [...]

   Test Failures (2):
   - test_email_validation: Expected True, got False
   - test_phone_validation: KeyError: 'country_code'

   Code has NOT been committed.

   Action Required:
   1. Fix the 3 lint errors
   2. Add missing type hints (5 functions)
   3. Fix the 2 failing tests
   4. Re-submit for verification

   Specification updated with urgent task at top.
   Please address these issues and the verification will run again.
   ```

8. Implementation Agent (or Owner):
   - Fixes all lint errors
   - Adds type hints
   - Fixes failing tests
   - REPORTS completion to Main Agent again

9. Main Agent:
   - Spawns Python Verification Agent again
   - Agent runs all checks
   - All checks PASS ✅
   - Main Agent updates tasks.md:
     * Marks urgent fix task as [x] completed
     * Marks original tasks as [x] completed
   - Commits code with verification status
   - Pushes to remote
   - Reports success to user
```

---

## Files Modified in This Integration

### Rule Files
- `.agents/rules/03-work-commit-rules.md` - Added verification status requirement
- `.agents/rules/04-coding-practice-agent-orchestration.md` - Complete rewrite (125 → 693 lines)
- `.agents/rules/05-git-auto-approval-and-push.md` - Added verification-aware workflows
- `.agents/rules/07-language-conventions-and-standards.md` - Created comprehensive rule

### Stack Files
- `.agents/stacks/rust.md` - Added ~400 line verification workflow section
- `.agents/stacks/javascript.md` - Added ~400 line verification workflow section
- `.agents/stacks/python.md` - Added ~420 line verification workflow section

### Configuration Files
- `AGENTS.md` - Updated with verification requirements
- `.agents/rules/08-verification-workflow-complete-guide.md` - This rule (comprehensive guide)

---

## Summary

**100% VERIFIED CODE**: Every commit is guaranteed to pass all quality gates.

**IRON-CLAD ENFORCEMENT**: No exceptions, no bypasses, zero tolerance.

**SELF-IMPROVING SYSTEM**: Learning Logs capture mistakes and improve standards over time.

**RACE CONDITION FREE**: ONE verification agent per stack prevents conflicts.

**COMPLETE INTEGRATION**: Rules 03, 04, 05, 07 and all stack files work together seamlessly.

---

*Created: 2026-01-11*
*Last Updated: 2026-01-11*
