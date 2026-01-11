# Coding Practice: Agent Orchestration and Mandatory Verification

## Purpose
This rule establishes the mandatory practice for code development, requiring specialized agent orchestration with **MANDATORY CODE VERIFICATION** before any commit. This is an **IRON-CLAD REQUIREMENT** with **ZERO TOLERANCE** for violations.

## Core Principles

### 1. Main Agent as Orchestrator
The Main Agent **MUST**:
- Act as controller and orchestrator ONLY
- NEVER perform coding tasks directly
- Launch specialized agents for all code work
- **MANDATORY**: Delegate to verification agents after implementation
- Coordinate specification updates
- Commit code ONLY after verification passes

### 2. Verification-First Workflow
**CRITICAL REQUIREMENT**: NO code is EVER committed without verification.

```
Implementation → Report to Main → Verification → Update Spec → Commit
```

## Mandatory Workflow

### Phase 1: Implementation

#### Main Agent Responsibilities
1. **Breaks down work** into specific tasks
2. **Identifies specifications** (references `.agents/specifications/NN-spec-name/`)
3. **Launches implementation agents** (up to 10 concurrent)
4. **WAITS for completion reports** from all agents
5. **DOES NOT COMMIT** anything yet

#### Implementation Agent Requirements
Each implementation agent **MUST**:

**Before Starting Work**:
1. ✅ Read `AGENTS.md` file
2. ✅ Load all rules from `.agents/rules/*`
3. ✅ Read specification `requirements.md` and `tasks.md`
4. ✅ Read relevant language stack files from `.agents/stacks/[language].md`
5. ✅ Understand what to build and standards to follow

**During Work**:
1. ✅ Write code following stack standards
2. ✅ Follow all conventions and best practices
3. ✅ Write tests for new functionality
4. ✅ Keep track of what was changed

**After Completing Work**:
1. ✅ **REPORT to Main Agent** (NEVER commit directly)
2. ✅ Provide list of changed files
3. ✅ Describe what was implemented
4. ✅ Note which language(s) were used
5. ✅ Reference specification if applicable
6. ✅ **STOP and WAIT** for Main Agent

**Implementation Agent MUST NOT**:
- ❌ Commit code directly
- ❌ Push to remote
- ❌ Update tasks.md directly
- ❌ Skip reporting to Main Agent
- ❌ Proceed without Main Agent approval

### Phase 2: Mandatory Verification (IRON-CLAD)

#### Main Agent Verification Orchestration

After receiving completion report from implementation agent, Main Agent **MUST**:

**Step 1: Identify Languages**
```
Main Agent analyzes changed files:
- Determine which language(s) were modified
- Identify relevant stack files (.agents/stacks/[language].md)
- Determine which verification agents to launch
```

**Step 2: Launch Verification Agents**

**CRITICAL RULE**: ONE verification agent per language stack (NEVER more than one per stack)

```
For each language modified:
  Main Agent spawns ONE [Language] Verification Agent

  Example:
  - Rust files changed → Spawn ONE Rust Verification Agent
  - TypeScript files changed → Spawn ONE JavaScript Verification Agent
  - Python files changed → Spawn ONE Python Verification Agent

  NEVER spawn multiple verification agents for the same language
  (prevents race conditions and file conflicts)
```

**Step 3: Provide Context to Verification Agents**

Main Agent provides each verification agent with:
```
Context Package:
- Changed files list (filtered by language)
- Implementation description
- Specification reference (.agents/specifications/NN-name/)
- Requirements.md location
- Tasks.md location
- Expected behavior
```

#### Verification Agent Execution

Each verification agent **MUST** execute ALL checks for their language:

##### Rust Verification Agent
**Must run in order**:
1. ✅ `cargo fmt -- --check` - Format verification
2. ✅ `cargo clippy -- -D warnings` - Lint (zero warnings)
3. ✅ `cargo test` - Run ALL tests OR specific crate tests
   ```bash
   # All tests
   cargo test

   # OR specific crate (if changes localized)
   cargo test --package crate-name
   ```
4. ✅ `cargo build` - Ensure successful build
   ```bash
   cargo build --all-features
   ```
5. ✅ `cargo doc --no-deps` - Documentation build
6. ✅ `cargo audit` - Security check
7. ✅ Standards compliance checks (see `.agents/stacks/rust.md`)

**On Success**: Report PASS to Main Agent
**On Failure**: Report FAIL with detailed errors to Main Agent

##### JavaScript/TypeScript Verification Agent
**Must run in order**:
1. ✅ `prettier --check .` - Format verification
2. ✅ `tsc --noEmit` - Type check (zero errors)
3. ✅ `eslint --max-warnings 0` - Lint (zero warnings)
4. ✅ `npm test` - Run tests with coverage
5. ✅ `npm run build` - Ensure successful build
6. ✅ `npm audit` - Security check
7. ✅ Standards compliance checks (see `.agents/stacks/javascript.md`)

**On Success**: Report PASS to Main Agent
**On Failure**: Report FAIL with detailed errors to Main Agent

##### Python Verification Agent
**Must run in order**:
1. ✅ `black --check .` - Format verification
2. ✅ `ruff check .` - Lint (zero errors)
3. ✅ `mypy .` - Type check (strict mode)
4. ✅ `pytest --cov` - Run tests with coverage
5. ✅ `python -m py_compile src/**/*.py` - Import check
6. ✅ `pip-audit` or `bandit` - Security check
7. ✅ Standards compliance checks (see `.agents/stacks/python.md`)

**On Success**: Report PASS to Main Agent
**On Failure**: Report FAIL with detailed errors to Main Agent

#### Verification Report Format

Each verification agent returns:
```markdown
# [Language] Verification Report

## Status: PASS ✅ / FAIL ❌

## Files Verified
- [list of files checked]

## Check Results
1. Format: PASS ✅ / FAIL ❌
2. Lint: PASS ✅ / FAIL ❌
3. Type Check: PASS ✅ / FAIL ❌
4. Tests: PASS ✅ / FAIL ❌ ([N] passed, [N] failed)
5. Build: PASS ✅ / FAIL ❌
6. Security: PASS ✅ / FAIL ❌
7. Standards: PASS ✅ / FAIL ❌

## Details
[Specific errors, warnings, or issues if any]

## Test Results
- Total: [N]
- Passed: [N]
- Failed: [N]
- Coverage: [N]%

## Blockers
[Issues preventing commit, if any]

## Recommendations
[Suggestions for improvement, if any]
```

### Phase 3: Main Agent Decision

Main Agent receives verification reports and **MUST** follow this logic:

#### If ALL Verifications PASS ✅

```
Main Agent MUST:
1. ✅ Receive PASS reports from all verification agents
2. ✅ Identify related specification (if applicable)
3. ✅ Spawn Specification Update Agent (NEVER update directly)
4. ✅ Provide Specification Agent with:
   - Verification report (PASS status)
   - Completed tasks list
   - Files changed
   - Implementation description
5. ✅ WAIT for Specification Agent to complete
6. ✅ Commit code AND specification updates following Rule 03
7. ✅ Include verification status in commit message:
      "Verified by [Language] Verification Agent(s): All checks passed"
8. ✅ Push to remote following Rule 05
9. ✅ Report success to user
```

##### Specification Update Process

**If work relates to a specification**:

```
Main Agent responsibilities:
1. Identify related specification directory
2. Spawn Specification Update Agent
3. Provide agent with:
   - Specification path (.agents/specifications/NN-spec-name/)
   - Verification report (full PASS report)
   - List of tasks that were completed
   - Implementation summary
4. WAIT for agent to complete updates
5. Review agent's completion report
6. Commit specification changes with code

Main Agent MUST NOT:
❌ Read tasks.md directly
❌ Update tasks.md directly
❌ Update frontmatter directly
❌ Mark tasks as complete directly
```

**Specification Update Agent responsibilities**:

```
Specification Update Agent MUST:
1. Read tasks.md from specification directory
2. Identify which tasks were completed (from Main Agent's context)
3. Mark tasks as [x] completed
4. Update frontmatter:
   - Increment 'completed' count
   - Decrement 'uncompleted' count
5. Add completion notes if needed
6. Save tasks.md
7. Report completion to Main Agent
```

**Example tasks.md update**:
```markdown
---
completed: 7  # was 5, now 7
uncompleted: 3  # was 5, now 3
tools:
  - Rust
  - Cargo
  - Clippy
---

# Feature Implementation - Tasks

## Implementation Tasks
- [x] Create base API structure
- [x] Implement authentication endpoint  ← Just completed
- [x] Add error handling  ← Just completed
- [ ] Add rate limiting
- [ ] Add monitoring
- [ ] Write integration tests
```

#### If ANY Verification FAILS ❌

```
Main Agent MUST:
1. ❌ Receive FAIL report from one or more verification agents
2. ❌ DO NOT COMMIT any code
3. ❌ DO NOT push to remote
4. ✅ Identify which verification(s) failed
5. ✅ Extract detailed error information from verification report
6. ✅ Identify related specification directory
7. ✅ Spawn Specification Update Agent (NEVER update directly)
8. ✅ Provide Specification Agent with:
   - Specification path (.agents/specifications/NN-spec-name/)
   - Full verification FAIL report
   - Failed checks details
   - Files affected
   - Recommendation for fix
9. ✅ WAIT for Specification Agent to create urgent task and verification.md
10. ✅ Report detailed failures to user
11. ✅ Provide fix recommendations

Main Agent MUST NOT:
❌ Update tasks.md directly
❌ Create verification.md directly
❌ Update frontmatter directly
❌ Add urgent tasks directly
```

##### Failed Verification Task Creation

**When verification fails**, Main Agent **MUST** delegate to Specification Update Agent.

**Specification Update Agent responsibilities when verification FAILS**:

```
Specification Update Agent MUST:
1. Read tasks.md from specification directory
2. Create verification.md file in same directory with full verification report:
   - Status: FAIL
   - Date/time of failure
   - Language(s) that failed
   - All failed checks with details
   - Error messages and line numbers
   - Files affected
   - Recommended fixes
3. Add NEW URGENT task at TOP of tasks.md:
   - Mark as highest priority
   - Reference verification.md for details
   - Include brief summary of issues
4. Update frontmatter:
   - Increment 'uncompleted' count by 1
5. Save tasks.md
6. Save verification.md
7. Report completion to Main Agent

IMPORTANT: verification.md is TRANSIENT:
- Created fresh each verification failure
- Overwritten on next verification failure
- Deleted on verification success
- Agent reads it to understand what to fix
```

**Example verification.md**:
```markdown
# Verification Report - FAILED

**Status**: FAIL ❌
**Date**: 2026-01-11 15:30:45
**Language**: Rust
**Specification**: .agents/specifications/03-user-authentication/

## Failed Checks

### 1. Clippy Lint - FAIL ❌
```
warning: using `unwrap()` on a `Result` value
  --> src/auth/token.rs:45:22
   |
45 |     let key = key_result.unwrap();
   |                          ^^^^^^^^
   |
   = note: `#[warn(clippy::unwrap_used)]` on by default
   = help: for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#unwrap_used

error: could not compile `auth-service` due to previous error; 2 warnings emitted
```

### 2. Tests - FAIL ❌
```
running 45 tests
test auth::test_generate_token ... ok
test auth::test_validate_token_expired ... FAILED
test auth::test_validate_token_invalid ... FAILED
...

failures:

---- auth::test_validate_token_expired stdout ----
thread 'auth::test_validate_token_expired' panicked at 'assertion failed: `(left == right)`
  left: `true`,
 right: `false`', tests/auth_tests.rs:67:5

---- auth::test_validate_token_invalid stdout ----
thread 'auth::test_validate_token_invalid' panicked at 'called `Result::unwrap()` on an `Err` value: InvalidToken', tests/auth_tests.rs:82:37

test result: FAILED. 43 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
```

## Files Affected
- src/auth/token.rs (line 45: unwrap() usage)
- tests/auth_tests.rs (line 67: wrong assertion)
- tests/auth_tests.rs (line 82: unwrap() in test)

## Recommended Fixes

1. **Replace unwrap() with proper error handling**:
   ```rust
   // Change from:
   let key = key_result.unwrap();

   // To:
   let key = key_result?;  // or handle error explicitly
   ```

2. **Fix test assertion in test_validate_token_expired**:
   - Expected result was inverted
   - Update assertion at line 67

3. **Fix test panic in test_validate_token_invalid**:
   - Should use `unwrap_err()` since we expect an error
   - Or use pattern matching

## Agent Action Required

Read this verification report, fix all issues listed above, then:
1. Mark the urgent fix task in tasks.md as complete
2. Continue with remaining tasks in tasks.md
3. Report completion to Main Agent
4. Main Agent will re-run verification

When verification passes, this verification.md file will be deleted.
```

**Example tasks.md with urgent task**:
```markdown
---
completed: 5
uncompleted: 6  # Incremented by 1 for urgent task
tools:
  - Rust
  - Cargo
  - Clippy
---

# User Authentication - Tasks

## URGENT: Failed Verification Tasks
- [ ] **FIX: Verification failures in user authentication**
  - Verification failed on 2026-01-11 15:30
  - Language: Rust
  - Failed checks: Clippy (1 error), Tests (2 failures)
  - See verification.md for detailed report
  - Files affected: src/auth/token.rs, tests/auth_tests.rs
  - Must fix before proceeding

## Implementation Tasks
- [x] Create base API structure
- [x] Implement authentication endpoint
- [ ] Add rate limiting
- [ ] Add monitoring
- [ ] Write integration tests
```

**Notification to Implementation Agent or User**:
```
Main Agent reports:

❌ VERIFICATION FAILED ❌

Verification Agent: [Language] Verification Agent
Status: FAIL
Failed Checks:
  - [Check 1]: [Details]
  - [Check 2]: [Details]

Test Failures:
  - [Test 1]: [Error]
  - [Test 2]: [Error]

Code has NOT been committed.

Action Required:
1. Fix the issues listed above
2. Re-run implementation
3. Verification will run again automatically
4. Code will only be committed after verification passes

Specification updated: .agents/specifications/NN-name/tasks.md
New urgent task added at top of task list.
```

### Phase 4: Fix and Retry (If Verification Failed)

#### Implementation Agent Fixes Issues

```
1. Main Agent spawns Implementation Agent (or resumes existing agent)
2. Main Agent provides context:
   - Specification path
   - verification.md location
   - Urgent task details
3. Implementation agent reads verification.md:
   - Understands all failed checks
   - Reviews error messages and line numbers
   - Identifies recommended fixes
4. Implementation agent fixes code issues:
   - Addresses ALL failures listed in verification.md
   - Ensures tests pass locally
   - Follows all stack standards
5. Implementation agent marks urgent fix task as [x] complete in tasks.md
6. Implementation agent REPORTS completion to Main Agent again
7. Main Agent launches verification agents again (back to Phase 2)
8. IF verification PASSES:
   - Specification Update Agent deletes verification.md
   - Marks completed tasks in tasks.md
   - Main Agent commits all changes
9. IF verification FAILS again:
   - Process repeats (Specification Agent updates verification.md)
   - New urgent task created (or existing one updated)
   - Implementation agent fixes again
```

**CRITICAL**: This loop continues until ALL verifications PASS. There is NO bypass.

**verification.md Lifecycle**:
- Created by Specification Update Agent on verification FAIL
- Read by Implementation Agent to understand fixes needed
- Overwritten on subsequent failures
- Deleted by Specification Update Agent on verification PASS

## Verification Agent Rules (IRON-CLAD)

### Mandatory Requirements

Each verification agent **MUST**:
1. ✅ Be spawned by Main Agent ONLY
2. ✅ Run ALL checks for their language (no skipping)
3. ✅ Be the ONLY verification agent for that language stack
4. ✅ Generate comprehensive report
5. ✅ Report to Main Agent ONLY
6. ✅ NEVER commit code (Main Agent delegates this)
7. ✅ NEVER update specifications (Main Agent delegates this)
8. ✅ NEVER create verification.md (Main Agent delegates this)

Each verification agent **MUST NOT**:
1. ❌ Skip any verification checks
2. ❌ Allow partial passes ("tests mostly pass")
3. ❌ Commit code directly
4. ❌ Update tasks.md directly
5. ❌ Create verification.md directly
6. ❌ Run concurrently with another agent for same language
7. ❌ Proceed when checks fail

### Race Condition Prevention

**CRITICAL RULE**: Only ONE verification agent per language stack at any time.

```
Good ✅:
  Main Agent sees Rust changes
  → Spawns ONE Rust Verification Agent
  → Waits for completion
  → Agent finishes
  → Reports back

Bad ❌:
  Main Agent sees Rust changes
  → Spawns Rust Verification Agent #1
  → Also spawns Rust Verification Agent #2
  → VIOLATION: Race condition possible!
  → File conflicts
  → Inconsistent results
```

**Prevention Strategy**:
```
Main Agent tracks active verification agents:
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
```

## Complete Workflow Example

### Successful Workflow ✅

```
1. User: "Implement user authentication in Rust"

2. Main Agent:
   - Reads specification: .agents/specifications/03-user-authentication/
   - Reads requirements.md and tasks.md
   - Identifies tasks to complete

3. Main Agent:
   - Spawns Rust Implementation Agent
   - Provides context (spec, requirements, tasks)

4. Rust Implementation Agent:
   - Reads AGENTS.md
   - Reads all rules from .agents/rules/*
   - Reads .agents/stacks/rust.md
   - Reads specification files
   - Implements authentication code
   - Writes tests
   - REPORTS completion to Main Agent:
     * Files changed: [list]
     * Language: Rust
     * Description: [what was built]
     * Specification: 03-user-authentication
   - STOPS and WAITS

5. Main Agent:
   - Receives completion report
   - Identifies language: Rust
   - Checks active_verifications['rust'] = null ✅
   - Spawns ONE Rust Verification Agent
   - Marks active_verifications['rust'] = agent-id-123
   - Provides context (files, spec, description)

6. Rust Verification Agent:
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

7. Main Agent:
   - Receives PASS report
   - Marks active_verifications['rust'] = null
   - Identifies specification: .agents/specifications/03-user-authentication/
   - Spawns Specification Update Agent
   - Provides context:
     * Verification report (PASS)
     * Completed tasks: authentication middleware, JWT generation, tests
     * Files changed: [list]

8. Specification Update Agent:
   - Reads .agents/specifications/03-user-authentication/tasks.md
   - Marks completed tasks as [x]:
     * [x] Implement authentication middleware
     * [x] Add JWT token generation
     * [x] Write authentication tests
   - Updates frontmatter: completed: 8, uncompleted: 2
   - Deletes verification.md if it exists (cleanup from previous failure)
   - Saves tasks.md
   - Reports completion to Main Agent

9. Main Agent:
   - Receives completion report from Specification Agent
   - git add [all changed files including specification]
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
   - git status (verify)
   - git push (automatic, Rule 05)

10. Main Agent:
   - Reports success to user:
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

### Failed Verification Workflow ❌

```
1. User: "Add data validation to API endpoints"

2. Main Agent:
   - Reads specification: .agents/specifications/05-data-validation/
   - Spawns Python Implementation Agent

3. Python Implementation Agent:
   - Implements validation logic
   - Writes some tests (but not all edge cases)
   - REPORTS completion to Main Agent

4. Main Agent:
   - Spawns ONE Python Verification Agent

5. Python Verification Agent:
   - Runs black -- --check → PASS ✅
   - Runs ruff check . → FAIL ❌ (3 lint errors)
   - Runs mypy . → FAIL ❌ (missing type hints)
   - Runs pytest → FAIL ❌ (2 tests failing)
   - Stops checking (failures detected)
   - Generates FAIL report with details

6. Main Agent:
   - Receives FAIL report
   - DOES NOT COMMIT code ❌
   - Identifies specification: .agents/specifications/05-data-validation/
   - Spawns Specification Update Agent
   - Provides context:
     * Verification report (FAIL with full details)
     * Failed checks summary
     * Files affected: src/validation.py, tests/test_validation.py
     * Recommended fixes

7. Specification Update Agent:
   - Reads .agents/specifications/05-data-validation/tasks.md
   - Creates verification.md with full FAIL report:
     * All lint errors with line numbers
     * All type errors
     * All test failures with stack traces
     * Recommended fixes
   - Adds NEW URGENT task at TOP of tasks.md:
     ```markdown
     ## URGENT: Failed Verification Tasks
     - [ ] **FIX: Verification failures in data validation**
       - Verification failed on 2026-01-11 15:30
       - Language: Python
       - Failed checks: Lint (3 errors), Type Check (5 errors), Tests (2 failures)
       - See verification.md for detailed report
       - Files affected: src/validation.py, tests/test_validation.py
       - Must fix before proceeding
     ```
   - Updates frontmatter: uncompleted: 4 → 5
   - Saves tasks.md and verification.md
   - Reports completion to Main Agent

8. Main Agent:
   - Receives completion report
   - DOES NOT commit anything ❌
   - DOES NOT push anything ❌
   - Reports to user:
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

     Specification updated: urgent task added, verification.md created
     Full details: .agents/specifications/05-data-validation/verification.md
     ```

9. Main Agent:
   - Spawns Implementation Agent (or resumes existing)
   - Provides context:
     * Specification path
     * verification.md location
     * Urgent task to complete

10. Implementation Agent:
   - Reads verification.md
   - Fixes all lint errors
   - Adds all missing type hints
   - Fixes both failing tests
   - Marks urgent fix task as [x] in tasks.md
   - REPORTS completion to Main Agent again

11. Main Agent:
   - Spawns Python Verification Agent again
   - Agent runs all checks
   - All checks PASS ✅

12. Main Agent:
   - Spawns Specification Update Agent
   - Provides context (PASS report, completed tasks)

13. Specification Update Agent:
   - Marks urgent fix task as [x] completed
   - Marks original validation tasks as [x] completed
   - Deletes verification.md (verification passed)
   - Updates frontmatter
   - Saves tasks.md
   - Reports completion

14. Main Agent:
   - Commits code and specification updates
   - Pushes to remote (Rule 05)
   - Reports success to user
```

## Integration with Other Rules

### Rule 03 (Work Commit Rules)
- Verification happens BEFORE commit
- Commit message includes verification status
- Only verified code is committed
- Specification updates included in commit

### Rule 05 (Git Auto-Approval and Push)
- Push happens AFTER successful verification
- No manual approval needed (verification is the gate)
- Automatic push on verification pass

### Rule 06 (Specifications and Requirements)
- Verification agent receives specification context
- Tasks.md updated based on verification results
- Failed verification creates urgent task
- Successful verification marks tasks complete

### Rule 07 (Language Conventions)
- Verification enforces all stack standards
- Stack files define verification commands
- Learning Logs updated with issues found
- Standards continuously improved

## Enforcement (ZERO TOLERANCE)

### Violations

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

### User Impact

Violations have severe consequences:
- ❌ **Broken builds** in production
- ❌ **Failed tests** discovered too late
- ❌ **Race conditions** from concurrent verifications
- ❌ **Lost work** from reverts
- ❌ **Wasted time** fixing avoidable issues
- ❌ **User frustration** and lost trust

**THE USER WILL BE VERY UPSET** if this workflow is not followed!

## Summary

**Core Workflow** (IRON-CLAD):
```
Implement → Report → Verify → Update Spec → Commit → Push
```

**Key Rules**:
1. ✅ Implementation agents NEVER commit directly
2. ✅ Main Agent ALWAYS delegates to verification
3. ✅ ONE verification agent per language stack (no more)
4. ✅ ALL checks must PASS before commit
5. ✅ Specifications updated based on verification results
6. ✅ Failed verification creates urgent task
7. ✅ Process repeats until verification passes

**Zero Tolerance**:
- ❌ No bypassing verification
- ❌ No skipping checks
- ❌ No partial passes
- ❌ No concurrent verifications per stack
- ❌ No committing on failure

**Result**: **100% VERIFIED CODE** - Every commit is guaranteed to pass all quality gates.

---
*Created: 2026-01-11*
*Last Updated: 2026-01-11*
